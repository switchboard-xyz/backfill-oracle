# Backfill Worker Function

Build the docker image:

```bash
docker buildx build --pull --platform linux/amd64 --tag gallynaut/backfill-oracle-worker --load ../
```

Publish the docker image:

```bash
docker buildx build --pull --platform linux/amd64 --tag gallynaut/backfill-oracle-worker --push ../
```
