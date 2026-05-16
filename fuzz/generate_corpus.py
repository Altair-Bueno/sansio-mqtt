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
    # PUBLISH — QoS1, dup=0, retain=0, topic "test", packet_id=10, no properties, payload "test"
    (
        "publish",
        bytes([
            0x32, 14,        # PUBLISH type=3, flags=0b0010 (QoS1), remaining=14
            0, 4,            # topic length
            116, 101, 115, 116,  # "test"
            0, 10,           # packet_id=10
            0,               # properties length=0
            116, 101, 115, 116,  # payload "test"
        ]),
    ),
    # PUBREC — packet_id=1, implicit success
    ("pubrec", bytes([0x50, 2, 0, 1])),
    # PUBREL — packet_id=1, implicit success
    ("pubrel", bytes([0x62, 2, 0, 1])),
    # PUBCOMP — packet_id=1, implicit success
    ("pubcomp", bytes([0x70, 2, 0, 1])),
    # SUBSCRIBE — packet_id=1, one topic filter "test" with QoS0
    (
        "subscribe",
        bytes([
            0x82, 11,        # SUBSCRIBE type=8, flags=0b0010, remaining=11
            0, 1,            # packet_id=1
            0,               # properties length=0
            0, 4,            # topic filter length
            116, 101, 115, 116,  # "test"
            0,               # subscription options (QoS0)
        ]),
    ),
    # SUBACK — packet_id=1, one reason code: Success(QoS0)
    (
        "suback",
        bytes([
            0x90, 4,         # SUBACK type=9, remaining=4
            0, 1,            # packet_id=1
            0,               # properties length=0
            0,               # reason code: Granted QoS0
        ]),
    ),
    # UNSUBSCRIBE — packet_id=1, one topic filter "test"
    (
        "unsubscribe",
        bytes([
            0xa2, 9,         # UNSUBSCRIBE type=10, flags=0b0010, remaining=9
            0, 1,            # packet_id=1
            0,               # properties length=0
            0, 4,            # topic filter length
            116, 101, 115, 116,  # "test"
        ]),
    ),
    # UNSUBACK — packet_id=1, one reason code: Success
    (
        "unsuback",
        bytes([
            0xb0, 4,         # UNSUBACK type=11, remaining=4
            0, 1,            # packet_id=1
            0,               # properties length=0
            0,               # reason code: Success
        ]),
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
