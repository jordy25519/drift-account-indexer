db = db.getSiblingDB('drift');
db.createUser({
  user: 'service',
  pwd: 'password',
  roles: [{ role: 'readWrite', db: 'drift' }],
});