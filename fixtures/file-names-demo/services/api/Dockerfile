# syntax=docker/dockerfile:1.7
FROM node:22-alpine AS deps
WORKDIR /srv/app
COPY package.json package-lock.json ./
RUN npm ci --omit=dev --ignore-scripts

FROM node:22-alpine AS runtime
ENV NODE_ENV=production
WORKDIR /srv/app
RUN addgroup -S app && adduser -S app -G app
COPY --from=deps /srv/app/node_modules ./node_modules
COPY dist/* ./dist/
USER app
HEALTHCHECK --interval=30s --timeout=3s CMD wget -qO- http://localhost:8080/health || exit 1
EXPOSE 8080
CMD ["node", "dist/api.js"]
