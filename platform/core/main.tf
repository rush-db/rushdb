terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 4.0"
    }
  }
}

# IAM =============================================================================================

resource "aws_iam_role" "ecs_task_execution_role" {
  # Creates an IAM role for ECS tasks to execute with the specified assume role policy.
  name               = "ecsTaskExecutionRole"
  assume_role_policy = data.aws_iam_policy_document.ecs_assume_role_policy.json
}

data "aws_iam_policy_document" "ecs_assume_role_policy" {
  # Defines an IAM policy allowing ECS tasks to assume the specified role.
  statement {
    actions = ["sts:AssumeRole"]

    principals {
      type        = "Service"
      identifiers = ["ecs-tasks.amazonaws.com"]
    }
  }
}

resource "aws_iam_role_policy_attachment" "cloudwatch_logs_access" {
  # Attaches the CloudWatch Logs Full Access policy to the ECS execution role.
  role       = aws_iam_role.ecs_task_execution_role.name
  policy_arn = "arn:aws:iam::aws:policy/CloudWatchLogsFullAccess"
}

resource "aws_iam_role_policy_attachment" "ecs_task_execution_policy" {
  # Attaches the Amazon ECS Task Execution Role policy to the ECS execution role.
  role       = aws_iam_role.ecs_task_execution_role.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
}

# CLOUDWATCH LOGS =================================================================================

resource "aws_cloudwatch_log_group" "rushdb_logs" {
  # Creates a CloudWatch log group for RushDB application logs.
  name              = "/ecs/rushdb"
  retention_in_days = 30

  tags = {
    Name        = "rushdb-logs"
    Environment = "production"
  }
}

# NETWORK =========================================================================================

data "aws_vpc" "default" {
  # Fetches the default VPC in the AWS account. Used to configure resources in the correct network.
  default = true
}

data "aws_subnets" "all" {
  # Retrieves all subnets associated with the default VPC.
  filter {
    name   = "vpc-id"
    values = [data.aws_vpc.default.id]
  }
}

resource "aws_security_group" "rushdb-security-group" {
  # Security group for the RushDB application, allowing all inbound and outbound traffic.
  name        = "rushdb-security-group"
  vpc_id      = data.aws_vpc.default.id

  ingress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1" # Allows all protocols.
    self        = true
    cidr_blocks = ["0.0.0.0/0"] # Allows traffic from all IPs.
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1" # Allows all protocols.
    cidr_blocks = ["0.0.0.0/0"] # Allows traffic to all IPs.
  }
}

resource "aws_security_group" "rushdb-task-security-group" {
  # Security group for ECS tasks, allowing communication with the RushDB security group.
  name        = "rushdb-task-security-group"
  vpc_id      = data.aws_vpc.default.id

  ingress {
    protocol        = "-1" # Allows all protocols.
    from_port       = 0
    to_port         = 0
    self            = true
    cidr_blocks     = ["0.0.0.0/0"]
    security_groups = [aws_security_group.rushdb-security-group.id] # Allows traffic from the RushDB security group.
  }

  egress {
    protocol    = "-1"
    from_port   = 0
    to_port     = 0
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_lb" "default" {
  # Creates an application load balancer for RushDB.
  name            = "rushdb-load-balancer"
  subnets         = data.aws_subnets.all.ids
  security_groups = [aws_security_group.rushdb-security-group.id]
}

resource "aws_lb_target_group" "rushdb-target-group" {
  # Defines a target group for the load balancer, forwarding traffic to port 3000.
  name        = "rushdb-target-group"
  port        = 3000
  protocol    = "HTTP"
  vpc_id      = data.aws_vpc.default.id
  target_type = "ip"
}

resource "aws_lb_listener" "rushdb-listener" {
  # Configures the load balancer to listen on port 80 and forward traffic to the target group.
  load_balancer_arn = aws_lb.default.id
  port              = "80"
  protocol          = "HTTP"

  default_action {
    type = "redirect"

    redirect {
      protocol = "HTTPS"
      port     = "443"
      status_code = "HTTP_301"
    }
  }
}

resource "aws_lb_listener" "https_listener" {
  # Adds an HTTPS listener to the load balancer using the validated SSL certificate.
  load_balancer_arn = aws_lb.default.arn
  port              = 443
  protocol          = "HTTPS"

  ssl_policy        = "ELBSecurityPolicy-2016-08"
  certificate_arn   = aws_acm_certificate_validation.rushdb_cert_validation.certificate_arn

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.rushdb-target-group.arn
  }
}

# ECS =============================================================================================

resource "aws_ecs_task_definition" "rushdb-task-definition" {
  # Defines an ECS task for the RushDB application, specifying resources, image, and logging configuration.
  family                   = "rushdb-task-definition"
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  cpu                      = 1024
  memory                   = 2048
  execution_role_arn       = aws_iam_role.ecs_task_execution_role.arn

  container_definitions = jsonencode([
    {
      name      = "rushdb"
      image     = "rushdb/platform:latest"
      cpu       = 1024
      memory    = 2048
      essential = true

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.rushdb_logs.name
          "awslogs-region"        = "eu-central-1"
          "awslogs-stream-prefix" = "ecs-rushdb"
        }
      }

      portMappings = [
        {
          containerPort = 3000
          hostPort      = 3000
          protocol      = "tcp"
          appProtocol   = "http"
        }
      ]
    }
  ])
}

resource "aws_ecs_cluster" "rushdb-ecs-cluster" {
  # Creates an ECS cluster for the RushDB tasks.
  name = "rushdb-ecs-cluster"
}

resource "aws_ecs_service" "rushdb-ecs-service" {
  # Deploys the RushDB task to the ECS cluster with a load balancer and network configuration.
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

  depends_on = [
    aws_lb_listener.rushdb-listener,
    aws_cloudwatch_log_group.rushdb_logs
  ]
}

# DOMAIN ==========================================================================================

# Fetch existing Route 53 zone
data "aws_route53_zone" "rushdb_zone" {
  # Fetches the Route 53 hosted zone for the domain rushdb.com.
  name         = "rushdb.com"
  private_zone = false
}

resource "aws_route53_record" "api_rushdb" {
  # Creates a DNS record for api.rushdb.com pointing to the load balancer.
  zone_id = data.aws_route53_zone.rushdb_zone.zone_id
  name    = "api.rushdb.com"
  type    = "CNAME"
  ttl     = 300
  records = [aws_lb.default.dns_name]
}

# Optional: SSL Certificate
resource "aws_acm_certificate" "rushdb_cert" {
  # Requests an SSL certificate for the RushDB API using DNS validation.
  domain_name       = "api.rushdb.com"
  validation_method = "DNS"

  tags = {
    Name = "rushdb-api-certificate"
  }
}

resource "aws_route53_record" "cert_validation" {
  # Creates DNS validation records for the SSL certificate.
  for_each = {
    for option in aws_acm_certificate.rushdb_cert.domain_validation_options : option.domain_name => {
      name  = option.resource_record_name
      type  = option.resource_record_type
      value = option.resource_record_value
    }
  }

  zone_id = data.aws_route53_zone.rushdb_zone.zone_id
  name    = each.value.name
  type    = each.value.type
  records = [each.value.value]
  ttl     = 300
}

resource "aws_acm_certificate_validation" "rushdb_cert_validation" {
  # Validates the SSL certificate using the DNS validation records.
  certificate_arn         = aws_acm_certificate.rushdb_cert.arn
  validation_record_fqdns = [for record in aws_route53_record.cert_validation : record.fqdn]
}

# OUTPUT ==========================================================================================

output "load_balancer_ip_rushdb" {
  # Outputs the DNS name of the load balancer.
  value = aws_lb.default.dns_name
}

output "execution_role_arn" {
  # Outputs the ARN of the ECS execution role.
  value = aws_iam_role.ecs_task_execution_role.arn
}

output "cloudwatch_log_group" {
  # Outputs the CloudWatch log group name for easy access to application logs.
  value = aws_cloudwatch_log_group.rushdb_logs.name
}
