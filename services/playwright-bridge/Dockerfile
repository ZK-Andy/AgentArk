FROM mcr.microsoft.com/playwright:v1.58.2-noble

WORKDIR /app

COPY package.json ./
RUN npm install --production

COPY index.js ./

EXPOSE 3100

CMD ["node", "index.js"]
