---
tags:
  - how-to
  - installation
  - curl
  - self-hosted
  - advanced
title: Install on Kubernetes
nextjs:
  metadata:
    title: Install on Kubernetes
    description: Install TerminusDB on a Kubernetes cluster
    keywords: terminusdb, getting started, install, install on kubernetes, kubernetes, setup
    openGraph:
      images: https://assets.terminusdb.com/docs/technical-documentation-terminuscms-og.png
    alternates:
      canonical: https://terminusdb.org/docs/install-on-kubernetes/
media: []
---

{% callout type="note" %}
**What you'll achieve**
By the end of this guide, you will have TerminusDB deployed and running on your Kubernetes cluster.
{% /callout %}

TerminusDB can also be deployed on Kubernetes. There are several strategies to do this. One is a high-availability solution using shared storage like NFS and one is a more simple strategy that is more performant on reads/writes by using one deployment at the cost of scalability. In this document, we will guide you on how to deploy using the latter strategy with a very minimal example.

## Prerequisites

We assume that you have `minikube` set up locally or have a Kubernetes cluster set up somewhere else that you can reach through `kubectl`. Therefore, when you run `kubectl apply -f some_yaml_file.yaml` we assume that it will deploy on a cluster, locally or remotely on a cloud provider.

## Creating a TerminusDB deployment

```yaml
apiVersion: apps/v1 # for versions before 1.9.0 use apps/v1beta2
kind: Deployment
metadata:
  name: terminusdb-server
spec:
  selector:
    matchLabels:
      app: terminusdb-server
  replicas: 1
  template:
    metadata:
      labels:
        app: terminusdb-server
    spec:
      terminationGracePeriodSeconds: 30
      nodeSelector:
        organization: terminusdb
      containers:
        - name: terminusdb-server
          image: terminusdb/terminusdb-server:latest
          tty: true
          stdin: true
          livenessProbe:
            httpGet:
              path: /api/ok
              port: 6363
            initialDelaySeconds: 30
            periodSeconds: 180
          ports:
            - containerPort: 6363
          envFrom:
            - secretRef:
                name: db-user-pass
          env:
            - name: TERMINUSDB_SERVER_PORT
              value: "6363"
            - name: TERMINUSDB_ENABLE_DASHBOARD
              value: "false"
            - name: TERMINUSDB_SERVER_NAME
              valueFrom:
                fieldRef:
                  fieldPath: metadata.name
            - name: TERMINUSDB_LOG_LEVEL
              value: DEBUG
            - name: TERMINUSDB_LOG_FORMAT
              value: json
            - name: TERMINUSDB_LRU_CACHE_SIZE
              value: "1200"
          volumeMounts:
            - name: pvc-storage
              mountPath: "/app/terminusdb/storage"
      volumes:
        - name: pvc-storage
          persistentVolumeClaim:
            claimName: pv-claim
```

Some interesting things to note in this example are that we set `TERMINUSDB_ENABLE_DASHBOARD` to false. If you are using TerminusDB in a Kubernetes deployment, you probably don’t want to expose the dashboard to other applications, unless you want your users to use the dashboard themselves. The `LOG_FORMAT` is set to `json` because this makes the logs easier to parse and search for in different cloud logging environments. We also inherit the ENV variables from a secret called `db-user-pass`. We assume that the environment variable `TERMINUSDB_ADMIN_PASS` is set to the appropriate password. If you are just playing around with this deployment, you could leave it out and it will default as `root`. But don’t do this for any application.

The livenessProbe will take every 180 seconds whether TerminusDB is still up by calling the `ok` endpoint. This is a lightweight endpoint that just returns 200 if the request succeeds.

We assume that a volume claim has been created for a specific cloud environment or for Minikube using the hostPath option.

## Creating a service for the deployment

In order to make the deployment easily available for other Kubernetes applications, we have to create a service.

The service can look like this:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: terminusdb-server
spec:
  selector:
    app: terminusdb-server
  ports:
    - protocol: TCP
      port: 6363
      targetPort: 6363
```

It will expose the TerminusDB server deployment on port 6363 with the DNS name http://terminusdb-server:6363. Any Kubernetes application in the same namespace will be able to access TerminusDB server this way.

## Testing the deployment

To test whether the deployment is available, we will be using the port-forwarding features of `kubectl`. To expose the TerminusDB service to your local computer, run:

`kubectl port-forward service/terminusdb-server 6363:6363`

This will expose TerminusDB locally on your computer on `localhost:6363`. You can see whether the deployment is successful by running `curl http://localhost:6363/api/info`. If the deployment is successful, it will return information about the version of TerminusDB running on the cluster. For instance:

```json
{"@type":"api:InfoResponse", "api:info":
{"authority":"terminusdb://system/data/User/anonymous", "storage": 
{"version":"2"}, "terminusdb": 
{"git_hash":"19029acffcd25c9277451aa30ee0ff4c3029ae67", 
"version":"12.0.5"}, "terminusdb_store": {"version":"0.19.8"}}, 
"api:status":"api:success"}
```

## Next steps

You've deployed TerminusDB on Kubernetes. To continue building your setup:

- [**Advanced Docker configuration**](/docs/docker-advanced-configuration/) — environment variables, CLI access, and production hardening
- [**Self-hosted production deployment**](/docs/self-hosted-installation/) — full guide for running TerminusDB in production
- [**Connect with an API key**](/docs/how-to-connect-dfrnt-hub/) — connect clients to your deployed instance programmatically