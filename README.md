# IronStream

Runs a websocket gateway so you can statelessly communicate with websocket clients using webhooks. Created for use with cloudflare workers.

```env
# HTTP/Websocket server port
PORT=1234
# Webhook authentication secret
AUTH_TOKEN=SECRET_TOKEN
```

## Instructions
1. There are channels defined by channel ID strings, clients connect to these channels, and you send messages to channels based on these IDs.
2. To send a webhook message to a channel, POST to `/webhook` with authorization header and body of `{channel: "1", data: "{some_json: 1}"}`
3. Clients can connect with `/ws?channels=1,2,3`
4. GET /stats for some information on current channels/stats


## Installation/Running

1. File for env vars: 

`sudo nano /etc/ironstream.env`

```env
PORT=1234
AUTH_TOKEN=SECRET_TOKEN
```

`sudo chmod 600 /etc/ironstream.env && sudo chown root:root /etc/ironstream.env`

2. Run as service

`sudo nano /etc/systemd/system/ironstream.service`

```systemd
[Unit]
Description=Ironstream
After=docker.service
Requires=docker.service

[Service]
Restart=always
RestartSec=5
StartLimitInterval=70
StartLimitBurst=5

EnvironmentFile=/etc/ironstream.env
ExecStart=/usr/bin/docker run --rm \
  --name ironstream \
  --cpus="2" \
  --memory="512m" --memory-swap="512m" \
  --read-only \
  --env-file /etc/ironstream.env \
  --cap-drop=ALL --cap-add=NET_BIND_SERVICE \
  --device=/dev/null \
  --init \
  --ulimit nofile=100000:100000 --ulimit nproc=64000 \
  --security-opt no-new-privileges \
  --ipc=none \
  --tmpfs /tmp \
  -p 3131:3131 \
  --health-cmd="curl -f http://localhost:${PORT}/stats || exit 1" \
  --health-interval=60s --health-timeout=5s --health-retries=3 \
  gc261/ironstream:latest
ExecStop=/usr/bin/docker stop ironstream

[Install]
WantedBy=multi-user.target
```

```
sudo systemctl daemon-reload && \
sudo systemctl enable ironstream && \
sudo systemctl restart ironstream && \
sudo systemctl status ironstream
```


View logs: `sudo journalctl -u ironstream -f`

## Updating

`docker pull gc261/ironstream:latest`

`sudo systemctl restart ironstream`