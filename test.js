fetch("http://localhost:3113/broadcast/magnaboy", {
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
.then(data => {
    if (data.error) {
        console.error('Error:', data.error);
    } else {
        console.log('Success:', data.data);
    }
})
.catch(err => console.error('Network error:', err)); 