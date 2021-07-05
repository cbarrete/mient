# How to

In `~/.config/mient/config.json`
```json
{
    "user": "john",
    "homeserver": "https://matrix.myserver.tld",
    "password_cmd": ["echo", "hunter2"],
    "device_id": "CHANGEMEXX"
}
```

Change the `device_id`, just make one up, it will one day be handled automatically.

Also the default room is hardcoded in `src/app.rs`, something might go wrong if
it isn't changed.

## Usage

- `Ctrl-p/n` to select the previous/next room
- `Up/Down/Home/End` to select the previous/next/first/last message
- `Del` to redact the selected message

Sending a message while another one is selected will send a reply.

Scrolling past the first message with `Up` or `Home` fetches older messages.

## TODO

- Manage access tokens and restore logins
- Manage device ids
- Respond to room key requests
- Custom message room widget
  - Timestamps
- External editor
- Read receipts
- Better room list (consistent ordering)
- Get initial state from the state store when it supports it again
- Media handler(s)
- Notifications handler
- Widget-local renders
