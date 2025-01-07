data "aws_vpc" "default" {
  default = true
}

data "aws_subnets" "all" {
  filter {
    name   = "vpc-id"
    values = [data.aws_vpc.default.id]
  }
}

resource "aws_iam_role" "ecs_task_execution_role" {
  name               = "ecsTaskExecutionRole"
  assume_role_policy = data.aws_iam_policy_document.ecs_assume_role_policy.json
}

data "aws_iam_policy_document" "ecs_assume_role_policy" {
  statement {
    actions = ["sts:AssumeRole"]

    principals {
      type        = "Service"
      identifiers = ["ecs-tasks.amazonaws.com"]
    }
  }
}

resource "aws_iam_role_policy_attachment" "cloudwatch_logs_access" {
  role       = aws_iam_role.ecs_task_execution_role.name
  policy_arn = "arn:aws:iam::aws:policy/CloudWatchLogsFullAccess"
}

resource "aws_iam_role_policy_attachment" "ecs_task_execution_policy" {
  role       = aws_iam_role.ecs_task_execution_role.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
}

resource "aws_security_group" "rushdb-security-group" {
  name        = "rushdb-security-group"
  vpc_id      = data.aws_vpc.default.id

  ingress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    self        = true
    cidr_blocks = ["0.0.0.0/0"]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_security_group" "rushdb-task-security-group" {
  name        = "rushdb-task-security-group"
  vpc_id      = data.aws_vpc.default.id

  ingress {
    protocol        = "-1"
    from_port       = 0
    to_port         = 0
    self            = true
    cidr_blocks     = ["0.0.0.0/0"]
    security_groups = [aws_security_group.rushdb-security-group.id]
  }

  egress {
    protocol    = "-1"
    from_port   = 0
    to_port     = 0
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_lb" "default" {
  name            = "rushdb-load-balancer"
  subnets         = data.aws_subnets.all.ids
  security_groups = [aws_security_group.rushdb-security-group.id]
}

resource "aws_lb_target_group" "rushdb-target-group" {
  name        = "rushdb-target-group"
  port        = 3000
  protocol    = "HTTP"
  vpc_id      = data.aws_vpc.default.id
  target_type = "ip"
}

resource "aws_lb_listener" "rushdb-listener" {
  load_balancer_arn = aws_lb.default.id
  port              = "80"
  protocol          = "HTTP"

  default_action {
    target_group_arn = aws_lb_target_group.rushdb-target-group.id
    type             = "forward"
  }
}

resource "aws_ecs_task_definition" "rushdb-task-definition" {
  family                   = "rushdb-task-definition"
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  cpu                      = 1024
  memory                   = 2048
  execution_role_arn       = aws_iam_role.ecs_task_execution_role.arn

    container_definitions = <<DEFINITION
[
        {
            "name": "rushdb",
            "image": "rushdb/platform:latest",
            "cpu": 1024,
            "memory": 2048,
            "essential": true,
            "logConfiguration": {
                "logDriver": "awslogs",
                "options": {
                    "awslogs-create-group": "true",
                    "awslogs-group": "/ecs/rushdb",
                    "awslogs-region": "eu-central-1",
                    "awslogs-stream-prefix": "ecs-rushdb"
                }
            },
            "portMappings": [
                {
                    "containerPort": 3000,
                    "hostPort": 3000,
                    "protocol": "tcp",
                    "appProtocol": "http"
                }
            ]
        }
    ]
DEFINITION

}


resource "aws_ecs_cluster" "rushdb-ecs-cluster" {
  name = "rushdb-ecs-cluster"
}

resource "aws_ecs_service" "rushdb-ecs-service" {
  name            = "rushdb-ecs-service"
  cluster         = aws_ecs_cluster.rushdb-ecs-cluster.id
  task_definition = aws_ecs_task_definition.rushdb-task-definition.arn
  desired_count   = 1
  launch_type     = "FARGATE"
  health_check_grace_period_seconds = 600

  network_configuration {
    security_groups  = [aws_security_group.rushdb-task-security-group.id]
    subnets          = data.aws_subnets.all.ids
    assign_public_ip = true
  }

  load_balancer {
    target_group_arn = aws_lb_target_group.rushdb-target-group.id
    container_name   = "rushdb"
    container_port   = 3000
  }

  depends_on = [aws_lb_listener.rushdb-listener]
}

output "load_balancer_ip_rushdb" {
  value = aws_lb.default.dns_name
}
