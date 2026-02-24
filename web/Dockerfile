# ClinicClaw Web — Next.js production build
FROM node:22-slim AS builder

WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm ci

COPY . .
RUN npm run build

# Runtime stage
FROM node:22-slim

WORKDIR /app
COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static
COPY --from=builder /app/public ./public

ENV PORT=3001
ENV HOSTNAME=0.0.0.0

EXPOSE 3001

CMD ["node", "server.js"]
