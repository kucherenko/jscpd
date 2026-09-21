import fp from 'fastify-plugin';

export default fp(async function dispatchMailer(app) {
  const outbox = [];
  app.decorate('mailer', {
    announce(recipient, trackingCode) {
      outbox.push({ recipient, subject: `Parcel ${trackingCode} is on its way` });
      return outbox.length;
    },
  });
});
