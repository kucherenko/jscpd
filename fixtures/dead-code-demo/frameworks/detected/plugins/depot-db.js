import fp from 'fastify-plugin';

export default fp(async function depotDatabase(app) {
  const parcels = new Map();
  app.decorate('depot', {
    store(parcel) {
      parcels.set(parcel.trackingCode, parcel);
      return parcel;
    },
    find(trackingCode) {
      return parcels.get(trackingCode) ?? null;
    },
  });
});
