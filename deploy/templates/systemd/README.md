# systemd

`rushdb.service` is the unit from
[Deployment](../../../docs/manual/operations/deployment.md#run-under-systemd),
with memory limits for a 4 vCPU / 8 GB host and sandboxing. On a host with
the `rushdb` binary in `/usr/local/bin`:

```bash
sudo useradd --system --home /var/lib/rushdb --shell /usr/sbin/nologin rushdb
sudo install -d -o rushdb -g rushdb -m 0750 /var/lib/rushdb
sudo install -d -m 0750 /etc/rushdb
sudo install -m 0600 rushdb.env.example /etc/rushdb/rushdb.env   # then set RUSHDB_TOKEN
sudo install -m 0644 rushdb.service /etc/systemd/system/rushdb.service
sudo systemctl daemon-reload
sudo systemctl enable --now rushdb
journalctl -u rushdb -f
```

Adjust `MemoryHigh` and `MemoryMax` to your host. If you enable WAL
archiving, add the archive directory to `ReadWritePaths`.
