---
sidebar_position: 5
---

# Transactions

[Transactions](../concepts/transactions.mdx) in RushDB ensure data consistency by grouping multiple operations into a single atomic unit. The Python SDK provides a simple and powerful way to work with transactions, allowing you to perform multiple related operations with guaranteed consistency.

## Overview

Transactions in RushDB enable you to:
- Perform multiple operations as a single atomic unit
- Ensure data consistency across related records
- Roll back changes automatically if any operation fails
- Prevent partial updates that could leave your data in an inconsistent state

## Working with Transactions

The RushDB Python SDK offers two ways to work with transactions:

### 1. Explicit Transaction Management

```python
from rushdb import RushDB

db = RushDB("RUSHDB_API_TOKEN", base_url="https://api.rushdb.com/api/v1")

# Start a transaction
tx = db.tx.begin()

try:
    # Perform operations within the transaction
    product = db.records.create(
        label="PRODUCT",
        data={"name": "Smartphone X", "price": 999.99},
        transaction=tx
    )

    inventory = db.records.create(
        label="INVENTORY",
        data={"productId": product.id, "stock": 100},
        transaction=tx
    )

    # Create a relationship between records
    product.attach(
        target=inventory,
        options={"type": "HAS_INVENTORY"},
        transaction=tx
    )

    # Commit the transaction once all operations are successful
    tx.commit()
    print("Transaction committed successfully")
except Exception as e:
    # Roll back the transaction if any operation fails
    tx.rollback()
    print(f"Transaction rolled back due to error: {e}")
```

### 2. Context Manager (with statement)

```python
# Using transaction as a context manager
try:
    with db.tx.begin() as tx:
        # Create an order record
        order = db.records.create(
            label="ORDER",
            data={"orderId": "ORD-12345", "total": 129.99},
            transaction=tx
        )

        # Create order items
        item1 = db.records.create(
            label="ORDER_ITEM",
            data={"productId": "PROD-001", "quantity": 2, "price": 49.99},
            transaction=tx
        )

        item2 = db.records.create(
            label="ORDER_ITEM",
            data={"productId": "PROD-002", "quantity": 1, "price": 30.01},
            transaction=tx
        )

        # Connect items to the order
        order.attach(
            target=[item1, item2],
            options={"type": "CONTAINS_ITEM"},
            transaction=tx
        )

        # Transaction is automatically committed when the block exits normally
except Exception as e:
    # Transaction is automatically rolled back if an exception occurs
    print(f"Transaction failed: {e}")
```

## Transaction Operations

The Transaction API provides the following operations:

### begin()

Starts a new transaction.

```python
tx = db.tx.begin()
```

### commit()

Commits all operations in the transaction.

```python
tx.commit()
```

### rollback()

Rolls back all operations in the transaction.

```python
tx.rollback()
```

## Supported Methods with Transactions

Most RushDB Python SDK methods support an optional `transaction` parameter. Here are some examples:

### Records API

```python
# Create a record within a transaction
record = db.records.create(
    label="USER",
    data={"name": "John Doe"},
    transaction=tx
)

# Update a record within a transaction
db.records.update(
    record_id=record.id,
    data={"status": "active"},
    transaction=tx
)

# Delete a record within a transaction
db.records.delete_by_id(
    id_or_ids=record.id,
    transaction=tx
)

# Find records within a transaction
users = db.records.find(
    query={"labels": ["USER"]},
    transaction=tx
)
```

### Relationships

```python
# Create a relationship within a transaction
db.records.attach(
    source=user.id,
    target=group.id,
    options={"type": "BELONGS_TO"},
    transaction=tx
)

# Remove a relationship within a transaction
db.records.detach(
    source=user.id,
    target=group.id,
    options={"typeOrTypes": "BELONGS_TO"},
    transaction=tx
)
```

## Complex Transaction Example

Here's a more complex example showing how to use transactions to ensure data consistency in an e-commerce scenario:

```python
def process_order(db, customer_id, items):
    # Start a transaction
    tx = db.tx.begin()

    try:
        # 1. Create the order record
        order = db.records.create(
            label="ORDER",
            data={
                "orderDate": datetime.now().isoformat(),
                "status": "processing",
                "totalAmount": sum(item["price"] * item["quantity"] for item in items)
            },
            transaction=tx
        )

        # 2. Retrieve the customer
        customers = db.records.find(
            query={"where": {"id": customer_id}},
            transaction=tx
        )

        if not customers:
            raise Exception(f"Customer {customer_id} not found")

        customer = customers[0]

        # 3. Connect order to customer
        customer.attach(
            target=order,
            options={"type": "PLACED_ORDER"},
            transaction=tx
        )

        # 4. Process each order item
        order_items = []
        for item in items:
            # 4.1. Check inventory
            inventory = db.records.find(
                query={
                    "labels": ["INVENTORY"],
                    "where": {"productId": item["productId"]}
                },
                transaction=tx
            )

            if not inventory or inventory[0]["stock"] < item["quantity"]:
                raise Exception(f"Insufficient stock for product {item['productId']}")

            # 4.2. Create order item
            order_item = db.records.create(
                label="ORDER_ITEM",
                data={
                    "productId": item["productId"],
                    "quantity": item["quantity"],
                    "price": item["price"],
                    "subtotal": item["price"] * item["quantity"]
                },
                transaction=tx
            )

            order_items.append(order_item)

            # 4.3. Update inventory
            db.records.update(
                record_id=inventory[0].id,
                data={"stock": inventory[0]["stock"] - item["quantity"]},
                transaction=tx
            )

        # 5. Connect order items to order
        order.attach(
            target=order_items,
            options={"type": "CONTAINS"},
            transaction=tx
        )

        # 6. Update order status
        order.update(
            data={"status": "confirmed"},
            transaction=tx
        )

        # Commit the transaction
        tx.commit()
        return {"success": True, "orderId": order.id}

    except Exception as e:
        # Roll back the transaction if any step fails
        tx.rollback()
        return {"success": False, "error": str(e)}
```

## Transaction Limitations

1. **Timeouts**: Transactions have a timeout period. Long-running transactions may be automatically aborted.

2. **Isolation Level**: RushDB uses the underlying Neo4j transaction isolation level, which is READ_COMMITTED.

3. **Nested Transactions**: Nested transactions are not supported. You should use a single transaction for a set of related operations.

4. **Transaction Size**: Very large transactions with many operations may impact performance. Consider breaking extremely large operations into smaller batches.

## Best Practices

1. **Keep transactions short** - Transaction locks are held until the transaction is committed or rolled back.

2. **Handle exceptions properly** - Always include exception handling to ensure transactions are properly rolled back.

3. **Use appropriate scope** - Only include necessary operations in a transaction.

4. **Consider using the context manager** - The context manager approach guarantees proper transaction handling.

5. **Avoid long-running transactions** - Long-running transactions can impact system performance.

6. **Don't mix transactional and non-transactional operations** - Keep all related operations within the transaction.

7. **Test transaction rollback scenarios** - Ensure your application properly handles transaction failures.

## Related Documentation

- [Transactions Concept](../concepts/transactions.mdx) - Learn more about how transactions work in RushDB
- [Record Operations](./records/create-records.md) - Record operations supporting transactions
- [Relationships](./relationships.md) - Working with relationships in transactions

