import * as mqtt_packet from "npm:mqtt-packet@9.0.0";
import {{ Buffer }} from 'node:buffer';

const packet = {packet};
const options = {options};
const pkt = mqtt_packet.generate(packet, options);
Deno.stdout.writeSync(pkt);
