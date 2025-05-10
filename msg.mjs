fetch("http://localhost:3131/broadcast/magnaboy", {
    method: "POST",
    headers: {
        "Content-Type": "application/json",
        authorization: "123"
    },
    body: JSON.stringify({
			id: '1344fsdfsdfds3234',
			type: 'translation',
			data: {
				message: 'testing',
				before_translation: '彼らの腎臓を取り上げましょう。',
				time: Date.now() + 1000
			}
		})
})
.then(res => res.json())
.then(data => console.log(data))
	.catch(err => console.error(err));


import { WebSocketServer } from 'ws';
const wss = new WebSocketServer({
	host: '0.0.0.0',
  port: 3131,
  perMessageDeflate: {
    zlibDeflateOptions: {
      // See zlib defaults.
      chunkSize: 1024,
      memLevel: 7,
      level: 3
    },
    zlibInflateOptions: {
      chunkSize: 10 * 1024
    },
    // Other options settable:
    clientNoContextTakeover: true, // Defaults to negotiated value.
    serverNoContextTakeover: true, // Defaults to negotiated value.
    serverMaxWindowBits: 10, // Defaults to negotiated value.
    // Below options specified as default values.
    concurrencyLimit: 10, // Limits zlib concurrency for perf.
    threshold: 1024 // Size (in bytes) below which messages
    // should not be compressed if context takeover is disabled.
  }
});

wss.on('connection', (ws) => {
	console.log('connection');
});