const WebSocket = require("ws");

const MESSAGES_TO_SEND = 100;
const CLIENTS_AMOUNT = 100;

const HOST = "localhost";
const PORT = 3131;
const AUTH_TOKEN = "123";
const CHANNEL_ID = "12345";

const WEBHOOK_URL = `http://${HOST}:${PORT}/webhook/${CHANNEL_ID}`;
const WS_URL = `ws://${HOST}:${PORT}/ws?channels=${CHANNEL_ID}`;

let totalMessagesReceived = 0;

for (let i = 0; i < CLIENTS_AMOUNT; i++) {
	const ws = new WebSocket(WS_URL);
	ws.on("message", (data) => {
		JSON.parse(data.toString());
		totalMessagesReceived++;
	});

	ws.on("error", (error) => {
		console.error("WebSocket error:", error);
	});
}

async function sendWebhook(data) {
	const response = await fetch(WEBHOOK_URL, {
		method: "POST",
		body: JSON.stringify(data),
		headers: {
			Authorization: `${AUTH_TOKEN}`,
			"Content-Type": "application/json",
		},
	});
	if (!response.ok) {
		throw new Error(`HTTP error: ${response.status}`);
	}
}

async function wrongAuthTest() {
	const response = await fetch(WEBHOOK_URL, {
		method: "POST",
		body: JSON.stringify({ message: "Test message" }),
		headers: {
			Authorization: "WRONG",
			"Content-Type": "application/json",
		},
	});
	if (response.ok) {
		throw new Error(`Wrong auth shouldve failed`);
	} else {
		console.log("OK: Wrong auth failed");
	}
}

async function main() {
	const CHUNK_SIZE = 5;

	const now = performance.now();
	for (let i = 0; i < MESSAGES_TO_SEND / CHUNK_SIZE; i++) {
		const chunk = [];
		for (let j = 0; j < CHUNK_SIZE; j++) {
			chunk.push(sendWebhook({ message: `Test message ${i}` }));
		}
		await Promise.all(chunk);
	}
	console.log("Time to send messages:", performance.now() - now);

	setTimeout(() => {
		if (totalMessagesReceived !== MESSAGES_TO_SEND * CLIENTS_AMOUNT) {
			throw new Error(
				`Messages received are not equal to messages sent ${totalMessagesReceived} !== ${MESSAGES_TO_SEND * CLIENTS_AMOUNT}`,
			);
		} else {
			console.log(
				`Sent ${MESSAGES_TO_SEND * CLIENTS_AMOUNT}, received ${totalMessagesReceived}`,
			);
		}

		process.exit(0);
	}, 500);
}

main();
wrongAuthTest();
