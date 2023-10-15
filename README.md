# NATProxy

TCP forward+proxy，tcp forward and http/https/socks5 Proxy。

> communication path：`user -tcp-> natpoxy server -forward-> natproxy client --> app`

## Overview

Used for NAT intranet penetration, and supports access to intranet services through: direct forwarding, proxy, and reverse proxy. 

The client and server directly support active and passive connection modes.

## Build

###  Source Code

```bash
git clone https://github.com/mark0725/natproxy
cd natproxy
cargo install --path .
```
### Using Docker

Build Docker image:

```
docker build -t natproxy:`sed -n '3'p Cargo.toml|sed 's/"//g'|awk '{print $3}'`-scratch -f Dockerfile.scratch .
```

## Install and start

### Tranditional

#### Start server

* Command line

```bash
natproxy --role server \ 
--listen 0.0.0.0:8001 \
--ca /<path-to-file>/ca.pem  \
--cert /<path-to-file>/server.pem \
--key /<path-to-file>/server.key \
--pass proxy
```
* Using config file

```bash
natproxy -c <path-to-config>/server.yaml
```

* Using environment variables config

```
NATPROXY_ROLE=server
NATPROXY_LISTEN=0.0.0.0:8001
NATPROXY_CA_CERT=./<path-to-file>/ca.pem
NATPROXY_CERT=./<path-to-file>/server.pem
NATPROXY_KEY=./<path-to-file>/server.key
NATPROXY_LOG_LEVEL=info
NATPOXY_MAPPINGS='[{"name":"tcp-forward", "mode":"tcp", "listen":"127.0.0.1:8005", "forward":"127.0.0.1:5000"}]'
```

#### Start client

* Command line

```bash
natproxy --role client \
-S 127.0.0.1:8001 \
--ca /<path-to-file>/ca.pem  \
--cert /<path-to-file>/client1.pem \
--key /<path-to-file>/client1.key
```
* Using config file

```bash
natproxy -c <path-to-config>/client.yaml
```

* Using environment variables config

```
NATPROXY_ROLE=client
NATPROXY_SERVER=127.0.0.1:8001
NATPROXY_CA_CERT=./<path-to-file>/ca.pem
NATPROXY_CERT=./<path-to-file>/client1.pem
NATPROXY_KEY=./<path-to-file>/client1.key
NATPROXY_LOG_LEVEL=info
```

> env params format:yaml params upcase and with NATPROXY_ prefix

#### 其它指令

```bash
natproxy --help
```

### Using Docker

* Start server

```bash
docker run -d --name natproxy \
  -p 8001:8001 \
  -p 8005:8005 \
  -e "NATPROXY_ROLE=server" \
  -e "NATPROXY_LISTEN=0.0.0.0:8001" \
  -e "NATPROXY_CA_CERT=/appuser/certs/ca.pem" \
  -e "NATPROXY_CERT=/appuser/certs/server.pem" \
  -e "NATPROXY_KEY=/appuser/certs/server.key" \
  -e "NATPROXY_MAPPINGS=[{\"name\":\"tcp-forward\", \"mode\":\"tcp\", \"listen\":\"0.0.0.0:8005\", \"forward\":\"127.0.0.1:5000\"}]" \
  -v "/natproxy/certs:/appuser/certs" \
  natproxy:<version>-scratch
  
docker run -ti --rm natproxy:<version>-scratch
```
* Start client

```bash
docker run -d --name natproxy \
  -e "NATPROXY_ROLE=client" \
  -e "NATPROXY_SERVER=127.0.0.1:8001" \
  -e "NATPROXY_CA_CERT=/appuser/certs/ca.pem" \
  -e "NATPROXY_CERT=/appuser/certs/server.pem" \
  -e "NATPROXY_KEY=/appuser/certs/server.key" \
  -v "/natproxy/certs:/appuser/certs" \
  natproxy:<version>-scratch
  
docker run -ti --rm natproxy:<version>-scratch
```

## Config File

### Server config file:

```yaml
#server，client
role: server

#server listen address
listen: 127.0.0.1:8100

#The trusted CA certificate file in PEM format used to verify the cert.
ca_cert: /<path-to-file>/ca.pem

#Certificate/key  used for mTLS between server/client nodes.
cert: /<path-to-file>/server.pem
key: /<path-to-file>/server.key

#proxy_on: [tcp, socks5, http, https, httpreverse, udp]
proxy_on: [tcp]

#http socks https proxy password
#proxy_pass:

# proxy mapping
mappings:
  #http proxy
  - name: web-proxy
    mode: http
    listen: 0.0.0.0:8200
    domain: localhost
    headers:
      - [proxy, +, x-forward-for, $client_ip]
      - [proxy, +, from, $url]
      - [+, key, value]
      - [-, etag]
      - [+, last-modified, aaaa]
      
  #socks5 proxy
  - name: socks5-proxy
    mode: socks5
    listen: 0.0.0.0:8300
    
  #forward tcp to 127.0.0.1:8000
  - name: tcp-forward
    mode: tcp
    listen: 0.0.0.0:8400
    forward: 127.0.0.1:8080

```

### Client config file

```yaml
#server，client
role: client

#NATProxy Server addr
server: 127.0.0.1:8091

#The trusted CA certificate file in PEM format used to verify the cert.
ca_cert: /<path-to-file>/ca.pem

#Certificate/key  used for mTLS between server/client nodes.
cert: /<path-to-file>/client1.pem
key: /<path-to-file>/client1.key
```

## mTLS Certificate/key

* create openssl config file: `openssl-ext.conf`

```
[ v3_server ]
basicConstraints = critical,CA:false
keyUsage = nonRepudiation, digitalSignature
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always,issuer:always
subjectAltName = @alt_names

[ v3_client ]
basicConstraints = critical,CA:false
keyUsage = nonRepudiation, digitalSignature
extendedKeyUsage = critical, clientAuth
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always,issuer:always

[ alt_names ]
DNS.1 = localhost
```

* CA Certificate

```bash
openssl genrsa -des3 -out ca.key 4096
openssl req -new -x509 -days 1000 -key ca.key -out ca.pem -subj "/C=/ST=/L=/O=/OU=/CN=root"
```

* Server Certificate:

```bash
openssl req -nodes -new -newkey rsa:4096 -keyout server.key -out server.csr -subj "/C=/ST=/L=/O=/OU=/CN=server"

openssl x509 -req -in server.csr -CA ca.pem -CAkey ca.key -CAcreateserial -out server.pem -days 1825 -sha256 -extensions v3_server -extfile ./openssl-ext.conf
```

* Client Certificate:

```bash
openssl req -nodes -new -newkey rsa:4096 -keyout client1.key -out client1.csr -subj "/C=/ST=/L=/O=/OU=/CN=client1"

openssl x509 -req -in client1.csr -CA ca.pem -CAkey ca.key -CAcreateserial -out client1.pem -days 1825 -sha256 -extensions v3_client -extfile ./openssl-ext.conf
```

> TLSv3 Certificate 
>
> display Certificate info:`openssl x509 -in server.pem -text -noout`

## Use Case

The client and server directly support active and passive connection modes.
1. Active mode, the server has a fixed IP. The server is responsible for monitoring, and the client connects to the server. After the connection channel is established, the server forwards the user request to the client, and the client accesses the application and returns the application's response data to the user through the server.
2. Passive mode, the client has a fixed IP. The client is responsible for monitoring, and the server connects to the client. After the connection channel is established, the server forwards the user request to the client. The client accesses the application and returns the application response data to the user through the server.

The difference between the two modes is the forwarding channel establishment stage. Once the forwarding channel is established, the subsequent communication process is the same.

Active mode is suitable for scenarios where the external network accesses the internal network, and the internal network is NAT.

Passive mode is suitable for scenarios where the internal network accesses the external network, especially when internal network users have restricted access through firewalls.

## RoadMap

- [x] mTLS natproxy server - client
- [ ] Multiple forward channels
- [ ] Loadblance
- [ ] Multiple natproxy client
- Forward connection mode
  - [x] active: client connect to server
  - [ ] passive: server connect to client

- Proxy
  - [x] Tcp  forward
  - [ ] Socks5 proxy
  - [ ] http proxy
  - [ ] https proxy
  - [ ] http reverse proxy
- [ ] IPv6 Support
- [ ] Admin api
- [x] TLSv3

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in NATProxy by you, shall be licensed as MIT, without any additional
terms or conditions.

