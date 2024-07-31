use crate::modes::weekelijkse_plezier::time_util::pretix_export_period;
use color_eyre::eyre::Error;
use futures_util::future::try_join_all;
use pretix_request::data_exporter::{
    DataExporter, ExportResponseSaleItem, OrderDataExportOrderItem,
};
use pretix_request::events::{Event, EventId};
use pretix_request::organizer::Organizer;
use pretix_request::PretixClient;
use std::collections::HashMap;
use std::num::ParseFloatError;
use std::str::FromStr;
use time::{OffsetDateTime, UtcOffset};
use tracing::info;

pub struct EventSummary {
    pub event_name: String,
    pub totals: OrderExportTotals,
    pub pdf: Vec<u8>,
    pub sale_items: Vec<ExportResponseSaleItem>,
    pub items: HashMap<String, f32>,
}

/// Run a Pretix export for all available events for the export period.
pub async fn pretix_totals(
    pretix_client: &PretixClient,
    export_period_start: OffsetDateTime,
    offset: UtcOffset,
) -> color_eyre::Result<HashMap<EventId, EventSummary>> {
    let (period_start, period_end) = pretix_export_period(export_period_start, offset)?;
    info!("Period end: {period_end}");

    // List all organizers we have access to,
    // within each organizer, list all events,
    // for each event, run an export and compute the totals
    let results = try_join_all(Organizer::list(pretix_client).await?.into_iter().map(
        |organizer| async move {
            try_join_all(
                Event::list(pretix_client, &organizer.slug)
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
                            let export_items = data_export
                                .orders
                                .into_iter()
                                .filter(|order_item| {
                                    order_item.datetime >= period_start
                                        && order_item.datetime <= period_end
                                })
                                .collect::<Vec<_>>();


                            // Compute totals
                            let totals = order_export_calc_totals(&export_items);

                            let pdf = DataExporter::export_order_data_pdf(
                                pretix_client,
                                organizer_id,
                                &event.slug,
                                period_start,
                                period_end,
                            )
                            .await?;

                            let totals_per_item = calc_order_totals_per_sale_item(&export_items)
                                .into_iter()
                                .map(|(item_id, item_value_sum)| {
                                    let sale_item = data_export.items
                                        .iter()
                                        .find(|sale_item| sale_item.id == item_id)
                                        .ok_or(Error::msg("Could not find sale item corresponding to sale item in order."))?;
                                    Ok::<_, Error>((sale_item.name.clone(), item_value_sum))
                                })
                                .collect::<Result<HashMap<_, _>, _>>()?;

                            let event_name = event
                                .name
                                .get("en")
                                .cloned()
                                .unwrap_or(event.slug.to_string().clone());

                            Ok::<_, Error>((event.slug, EventSummary {
                                event_name,
                                sale_items: data_export.items,
                                totals,
                                pdf,
                                items: totals_per_item,
                            }))
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

pub fn calc_order_totals_per_sale_item(orders: &[OrderDataExportOrderItem]) -> HashMap<u32, f32> {
    let positions = orders
        .iter()
        .map(|order| {
            // Map the ordered items to map with K = item id and V = item price
            order
                .ordered_items
                .iter()
                .map(|v| (v.item, v.price))
                .collect::<HashMap<_, _>>()
        })
        .collect::<Vec<_>>();

    // We now have a list of maps.
    // Iterate over it and turn it into one map with K = item id and V = sum of prices for that item
    let mut totals = HashMap::new();
    for pos in positions {
        for (item_id, item_value) in pos {
            totals
                .entry(item_id)
                .and_modify(|values| *values += item_value)
                .or_insert(item_value);
        }
    }

    totals
}

#[derive(Debug)]
pub struct OrderExportTotals {
    /// The total amount without VAT or fees
    pub value: f32,
    /// The total fees without VAT or fees
    pub fees: f32,
}

/// Calculate the totals for the provided set of order items.
pub fn order_export_calc_totals(items: &[OrderDataExportOrderItem]) -> OrderExportTotals {
    let (value, fees) = items
        .iter()
        .map(|item| {
            let fees = item.fees.iter().map(|fee_item| fee_item.value).sum::<f32>();

            (item.total - fees, fees)
        })
        .collect::<Vec<(f32, f32)>>()
        .into_iter()
        .fold((0f32, 0f32), |(acc_value, acc_fee), (value, fee)| {
            (acc_value + value, acc_fee + fee)
        });

    OrderExportTotals { value, fees }
}
