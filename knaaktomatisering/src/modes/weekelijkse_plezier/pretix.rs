use crate::modes::weekelijkse_plezier::time_util::pretix_export_period;
use color_eyre::eyre::Error;
use futures_util::future::try_join_all;
use pretix_request::data_exporter::{DataExporter, OrderDataExportOrderItem};
use pretix_request::events::Event;
use pretix_request::organizer::Organizer;
use pretix_request::PretixClient;
use std::collections::HashMap;
use std::num::ParseFloatError;
use std::str::FromStr;
use time::{OffsetDateTime, UtcOffset};

pub struct EventSummary {
    pub totals: OrderExportTotals,
    pub pdf: Vec<u8>,
}

/// Run a Pretix export for all available events for the export period.
pub async fn pretix_totals(
    pretix_client: &PretixClient,
    export_period_start: OffsetDateTime,
    offset: UtcOffset,
) -> color_eyre::Result<HashMap<String, EventSummary>> {
    let (period_start, period_end) = pretix_export_period(export_period_start, offset)?;

    // List all organizers we have access to,
    // within each organizer, list all events,
    // for each event, run an export and compute the totals
    let results = try_join_all(Organizer::list(&pretix_client).await?.into_iter().map(
        |organizer| async move {
            try_join_all(
                Event::list(&pretix_client, &organizer.slug)
                    .await?
                    .into_iter()
                    // We do not need to check closed events
                    .filter(|event| event.live)
                    .map(|event| {
                        let organizer_id = &organizer.slug;
                        async move {
                            // Get all orders of this event
                            let data_export = DataExporter::export_order_data(
                                pretix_client,
                                organizer_id,
                                &event.slug,
                            )
                            .await?;

                            // Keep only those within our export period
                            let data_export = data_export
                                .into_iter()
                                .filter(|order_item| {
                                    order_item.datetime >= period_start
                                        && order_item.datetime <= period_end
                                })
                                .collect::<Vec<_>>();

                            // Compute totals
                            let totals = order_export_calc_totals(&data_export)?;

                            let pdf = DataExporter::export_order_data_pdf(
                                &pretix_client,
                                organizer_id,
                                &event.slug,
                                period_start.clone(),
                                period_end.clone(),
                            )
                            .await?;

                            let key = event
                                .name
                                .get("en")
                                .map(|name| name.clone())
                                .unwrap_or(event.slug.to_string().clone());

                            Ok::<_, Error>((key, EventSummary { totals, pdf }))
                        }
                    }),
            )
            .await
        },
    ))
    .await?
    .into_iter()
    .flatten()
    .collect::<HashMap<_, _>>();

    Ok(results)
}

#[derive(Debug)]
pub struct OrderExportTotals {
    /// The total amount without VAT or fees
    pub value: f32,
    /// The total fees without VAT or fees
    pub fees: f32,
}

/// Calculate the totals for the provided set of order items.
///
/// # Errors
///
/// If the order items contains string which cannot be parsed to floats
pub fn order_export_calc_totals(
    items: &[OrderDataExportOrderItem],
) -> Result<OrderExportTotals, ParseFloatError> {
    let (value, fees) = items
        .iter()
        .map(|item| {
            let value = f32::from_str(&item.total)?;
            let fees = item
                .fees
                .iter()
                .map(|fee| f32::from_str(&fee.value))
                .collect::<Result<Vec<_>, ParseFloatError>>()?
                .into_iter()
                .sum::<f32>();

            Ok((value - fees, fees))
        })
        .collect::<Result<Vec<(f32, f32)>, ParseFloatError>>()?
        .into_iter()
        .fold((0f32, 0f32), |(acc_value, acc_fee), (value, fee)| {
            (acc_value + value, acc_fee + fee)
        });

    Ok(OrderExportTotals { value, fees })
}
