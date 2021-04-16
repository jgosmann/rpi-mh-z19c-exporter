# rpi-mh-z19c-exporter

Export CO₂ readings from a
[Winsen MH-Z19C](https://www.winsen-sensor.com/sensors/co2-sensor/mh-z19c.html)
CO₂ sensor connected to a Raspberry Pi to Prometheus.
The sensor needs to be connected via the UART interface.

The following metrics will be provided at the `http://localhost:1202/metrics`
endpoint:

* `co2_ppm`: Measured CO₂ concentration in parts per million (ppm).

Mnemonic for the port: 12 is C in hexadecimal and 0 = O, thus 1202 = CO2.


## Implementation

The exporter is implemented with asynchronously [Tokio](https://tokio.rs/) using
a single thread. A request to the `/metrics` endpoint will trigger reading a
new measurement from the sensor. Multiple concurrent requests will only trigger
a single measurement wait for that one to complete.


## Installation

1. Download the binary from GitHub or compile one yourself.
2. Place it on your Raspberry Pi, e.g. in `/usr/local/bin/rpi-mh-z19c-exporter`
   and ensure the the executable permission is set.
3. Use the Ansible role provided in the `roles` directory to setup a service
   user and add a systemd service. (Or do this manually if you prefer.)


## Configuration

The exporter is configured with environment variables:

* `RPI_MHZ19C_UART_PATH`: Path to UART device that the sensor is connected to.
  Default: `/dev/ttyAMA0`
* `RPI_MHZ19C_EXPORTER_LISTEN_ADDRS`: Addresses (separated by space) to listen
  on. Default: `localhost:1202`
