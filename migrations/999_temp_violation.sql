CREATE TABLE test_violation (
    id SERIAL PRIMARY KEY,
    amount DECIMAL(20, 10) NOT NULL -- Should fail lint
);
