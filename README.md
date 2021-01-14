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

Change the `device_id`, just make one up, it will later be handled automatically.

# TODO

- Manage access tokens and restore logins
- Manage device ids
- Respond to room key requests
- Device verification
- Much better message handling, probably with a VecDeque for easy backfilling
- Custom message room widget
  - Nice colors
  - Timestamps
  - Selection (for replies, redactions and edits)
  - Keep track of max prefix (username/userid/ts) width and align messages
- External editor
- Read receipts
- Better room list (consistent ordering)
- Remove hardcoded default room
