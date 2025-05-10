// fetch("http://localhost:3131/broadcast/magnaboy", {
//     method: "POST",
//     headers: {
//         "Content-Type": "application/json",
//         authorization: "123"
//     },
//     body: JSON.stringify({
// 			id: '1344fsdfsdfds3234',
// 			type: 'translation',
// 			data: {
// 				message: 'testing',
// 				before_translation: '彼らの腎臓を取り上げましょう。',
// 				time: Date.now() + 1000
// 			}
// 		})
// })
// .then(res => res.json())
// .then(data => console.log(data))
// 	.catch(err => console.error(err));

import WebSocket from 'ws';
const ws = new WebSocket('ws://0.0.0.0:3131/ws');

ws.on('error', console.error);

ws.on('open', function open() {
  ws.send('something');
});

ws.on('message', function message(data) {
  console.log('received: %s', data);
});