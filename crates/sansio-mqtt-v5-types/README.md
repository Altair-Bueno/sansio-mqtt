MQTT v5.0 wire-level types, auxiliary types, parsers, and encoders for the
`sansio-mqtt` project.

This crate provides the value types of the MQTT v5.0 control packets plus
[`winnow`](::winnow)-based parsers and [`encode`](::encode)-based encoders. It
is `no_std`, although it does require the [`alloc`](::alloc) crate.

# Specification

This crate targets MQTT Version 5.0, OASIS Standard 07 March 2019:
<https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html>. Conformance
statements from Appendix B of that specification are cited using the verbatim
`[MQTT-X.Y.Z-N]` labels used in the spec.
