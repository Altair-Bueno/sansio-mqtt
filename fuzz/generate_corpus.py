#!/usr/bin/env python3
"""Generate binary seed corpus files from known-good MQTT v5 packet byte vectors."""
import os

SEEDS = [
    # PINGREQ — type 12, no payload
    ("pingreq", bytes([0xC0, 0x00])),
    # PINGRESP — type 13, no payload
    ("pingresp", bytes([0xD0, 0x00])),
    # CONNACK — success, no session, no properties
    ("connack_success", bytes([32, 3, 0, 0, 0])),
    # PUBACK — packet id 42, implicit success
    ("puback", bytes([64, 2, 0, 42])),
    # DISCONNECT — normal disconnection, no remaining
    ("disconnect", bytes([224, 0])),
    # CONNECT — MQTT5, clean start, receive-maximum=20, empty client id
    (
        "connect_minimal",
        bytes([16, 16, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 3, 33, 0, 20, 0, 0]),
    ),
]

TARGETS = ["parse_control_packet", "roundtrip_control_packet"]

for target in TARGETS:
    os.makedirs(f"corpus/{target}", exist_ok=True)
    for name, data in SEEDS:
        path = f"corpus/{target}/{name}.bin"
        with open(path, "wb") as f:
            f.write(data)

print(f"Wrote {len(SEEDS)} seeds × {len(TARGETS)} targets.")
