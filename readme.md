```shell
docker build  -t sndra-link:latest .

docker run -d \
-e DATABASE_URL=sqlite:///data/sndra-link.db \
-e DOMAIN=https://sndra.link \
-v $(pwd)/data:/app/data \
-p 7700:7700 \
--name sndra-link-service \
sndra-link



```
