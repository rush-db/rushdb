# @rushdb/zod

Adapter to use Zod schemas with the Standard Schema V1 interface.

```ts
import { z } from 'zod';
import { zodToStandard } from '@rushdb/zod';
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('')

const createMany = db.records.createMany.bind(db)

const User = z.object({ id: z.number(), name: z.string() });
const schema = zodToStandard(User);

const res = await createMany(schema, [{ id: 1, name: 'Ada' }]);
```
