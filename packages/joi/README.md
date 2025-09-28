# @rushdb/joi

Adapter to use Joi schemas with the Standard Schema V1 interface.

```ts
import Joi from 'joi';
import { joiToStandard } from '@rushdb/joi';
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('')

const createMany = db.records.createMany.bind(db)
type User = { id: number; name: string };
const joiUser = Joi.object<User>({ id: Joi.number().required(), name: Joi.string().required() });
const schema = joiToStandard<User>(joiUser);

const res = await createMany(schema, [{ id: 1, name: 'Ada' }]);
```
