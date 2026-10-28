# cloud-init (Linux VMs)

`cloud-init.yaml` turns a fresh Ubuntu or Debian VM into a RushDB host. Paste
it as **user data** when you create the machine:

| Provider | Where |
|---|---|
| DigitalOcean | Create Droplet → Advanced options → Add initialization scripts |
| Hetzner Cloud | Create Server → Cloud config |
| AWS EC2 | Launch instance → Advanced details → User data |
| Google Compute Engine | Metadata key `user-data` |
| Azure | Create VM → Advanced → Custom data |

It installs the `rushdb` binary with the install script, creates the
`rushdb` user and `/var/lib/rushdb`, writes `/etc/rushdb/rushdb.env` with a
random token, and starts [`rushdb.service`](../systemd/rushdb.service).

After boot:

```bash
ssh you@host 'sudo rushdb health'
ssh you@host 'sudo cat /etc/rushdb/rushdb.env'     # the token
ssh -L 3000:127.0.0.1:3000 -L 5432:127.0.0.1:5432 you@host   # reach it locally
```

The node listens on `127.0.0.1`. To serve other machines, put a TLS proxy
such as Caddy or nginx in front of port 3000, after reading
[Security](../../../docs/manual/operations/security.md). Allow only SSH and
the proxy's ports in the provider's firewall.
