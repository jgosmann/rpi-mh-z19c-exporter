# rpi-mh-z19c-exporter Ansible role

This Ansible role allows to setup the rpi-mh-z19c-exporter automatically.

To use this role, add

```yaml
- src: https://github.com/jgosmann/rpi-mh-z19c-exporter.git
```

to your `requirements.yml` and then install the role with:

```bash
ansible-galaxy install -r requirements.yml
```

You can then use the role in your playbooks like so:

```yaml
- hosts: all
  roles:
    - role: rpi-mh-z19c-exporter
      vars:
        binary_path: /usr/local/bin/rpi-mh-z19c-exporter
```

## Available role variables

* `binary_path` (string, default: `"rpi-mh-z19c-exporter"`):
  Path to the rpi-mh-z19c-exporter binary.
* `listen_addrs` (list of strings, default: `["localhost:1202"]`):
  Addresses to listen on.
* `sensor_path` (string, default: `"/dev/ttyAMA0"`):
  Path to the UART device connected to the sensor.
