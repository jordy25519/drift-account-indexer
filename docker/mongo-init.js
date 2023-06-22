db.createUser({
  user: 'service',
  pwd: 'password',
  roles: [
    { role: 'readWrite', db: 'drift' },
  ],
});
db.accounts.createIndex({ "address": "hashed "}); // this is unsupported.., TODO: make address a string