# **Local HTTP Proxy**

A simple command-line tool for routing local web traffic. Give your services memorable names instead of juggling
localhost ports.

## **Quick Start Guide**

**1. Add your services as routes:**

The `name` (api, frontend) becomes the URL segment. The `target` is where the service is actually running.

### Forwards requests from `/api` -> `localhost:8000`

```shell
local-http-proxy add api localhost:8000
```

### Forwards requests from `/frontend` -> `localhost:3000`

```shell
local-http-proxy add frontend localhost:3000
```

**2. Start the proxy server:**

The server will now listen on localhost (port 8080 by default). You may need sudo for privileged ports.

```shell
local-http-proxy start
```

**3. Access your services:**

You can now use the clean URLs in your browser or application:

* ✅ http://localhost/api
* ✅ http://localhost/frontend

## **Command Reference**

| Command             | Description                                         |
|:--------------------|:----------------------------------------------------|
| start               | Starts the proxy server. Use --port to override 80. |
| add `name` `target` | Creates or updates a routing rule.                  |
| remove `name`       | Deletes a routing rule.                             |
| list                | Shows all current routes and the active mode.       |
| set-mode `mode`     | Switches the routing mode (path or domain).         |

## **Using Domain Mode (Optional)**

If you prefer http://api.local over http://localhost/api, you can use domain mode.

**1. Switch the mode:**

local-http-proxy set-mode domain

**2. Edit your hosts file (requires administrator/sudo access):**

You must map your custom domains to your local machine.

* **macOS/Linux:** /etc/hosts
* **Windows:** C:\\Windows\\System32\\drivers\\etc\\hosts

Add entries for each route name:

Add these lines to your hosts file  
127.0.0.1 api.local
127.0.0.1 frontend.local

**3. Access your services:**

After starting the server, you can now use the domain-style URLs:

* ✅ http://api.local
* ✅ http://frontend.local
