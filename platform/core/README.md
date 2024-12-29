## Running the app

```bash
# 1. install dependencies
$ cd app & yarn

# 2. return to the root
$ cd ..

# 3. start the app
$ docker-compose -f docker-compose.dev.yml up
```

### Generate aws ecs task definition
```bash
cat docker-compose.yml | container-transform -v > aws/task-definition.json
```


### To deploy / update task in ECR
- Run the command `terraform init` from the project root directory. This will create the necessary `.terraform` folder.
- Run `terraform plan -out="tfplan"` to make a plan to launch, this will also provide you with the details of what AWS resources will be created.
- If you are satisfied with the plan then run `terraform apply "tfplan"`
- (Additional) To destroy the planned resources run `terraform plan -destroy -out=tfplan`
