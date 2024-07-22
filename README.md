# Knaaktomatisering
De automatisering van knaakzaken.

## Goal
The goal of this CLI is to automate certain tasks of the treasurer at S.V. Sticky.

Current state of affairs:
- [ ] Automatically add Pretix order exports to an Exact sale booking
    - [x] Run Pretix exports in PDF and JSON format on all live events
    - [ ] Insert the result into Exact
    
## SSL

To connect with Exact you need to use OAuth, which requires HTTPS. We only use localhost as redirect URI, however, this still needs 
to be an HTTPS url. Do achieve this without being too annoying, I use a self-signed certificate:
```bash
sudo apt install -y libnss3-tools mkcert
mkcert --install
mkcert knaaktomatisering.local
```
This will add mkcert's CA cert to the system's trust store and generate a certificate for `https://knaaktomatisering.local`.

## Sudo
The built-in webserver used for OAuth2 wants to bind on port 443, so you must either run this program with sudo or grant the required cap:
```bash
sudo setcap CAP_NET_BIND_SERVICE=+eip /path/to/binary
```
Alternatively, you can run only the authentication as root, and the rest as a regular user.
You can do this by running the program as root once with the `--only-auth` flag provided. The program
will then only perform authorizations. You can then run the program as a normal user (without the `--only-auth` flag), it will
then function properly as long as all authorization tokens are valid.