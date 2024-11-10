# Config format

```yaml
# Version defines the version of the format of the config file.
# Wrusrun checks this value to ensure compatibility.
version: 0.1

servers:
    - name: my-server
      listen: localhost:8080
      routes:
        - 
      tls:
        cert: /path/to/cert.pem
        key: /path/to/key.pem
      acme:
        emails:
            - one@example.com
            - other@example.com

```
