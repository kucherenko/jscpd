export default async function parcelRoutes(app) {
  app.post('/parcels', async (request, reply) => {
    const parcel = app.depot.store(request.body);
    app.mailer.announce(parcel.recipient, parcel.trackingCode);
    return reply.code(201).send(parcel);
  });

  app.get('/parcels/:trackingCode', async (request, reply) => {
    const parcel = app.depot.find(request.params.trackingCode);
    return parcel ? parcel : reply.code(404).send({ error: 'unknown parcel' });
  });
}
