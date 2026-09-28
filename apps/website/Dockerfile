FROM node:22-alpine AS build
WORKDIR /site
COPY package.json package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY index.html styles.css main.js ./
COPY scripts ./scripts
COPY src ./src
COPY public ./public
ARG SOURCE_REVISION=development
ENV SOURCE_REVISION=$SOURCE_REVISION
# The version to show (the image has no root package.json to read): the release checked on
# GitHub Releases, e.g. --build-arg PLENIPO_VERSION=1.10.0. The build stops without it.
ARG PLENIPO_VERSION
ENV PLENIPO_VERSION=$PLENIPO_VERSION
RUN node scripts/build.mjs

FROM nginx:1.30.5-alpine
ARG SOURCE_REVISION=development
LABEL org.opencontainers.image.title="Plenipo marketing website" \
      org.opencontainers.image.source="https://github.com/Seckcey/plenipo" \
      org.opencontainers.image.revision=$SOURCE_REVISION
COPY nginx.conf /etc/nginx/nginx.conf
COPY --from=build /site/dist /usr/share/nginx/html
USER 10001:10001
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD wget -q -O /dev/null http://127.0.0.1:8080/healthz || exit 1
ENTRYPOINT ["nginx", "-g", "daemon off;"]
