export const ts_1 = `// Simple as that
const db = new RushDB("API_TOKEN")`

export const curl_1 = `curl 'https://api.rushdb.com/api/v1/...' \\
  -H 'Content-Type: application/json' \\
  -H "Token: $API_TOKEN" \\
  -d '...' \\
`
export const ts_1_1 = `// Optionally define Models 
const UserRepo = new Model(
  "USER",
  {
    name: { type: 'string' },
    email: { type: 'string', uniq: true },
    age: { type: 'number', required: false },
    permissions: { type: 'string', multiple: true },
    createdAt: { 
      type: 'datetime',
      default: new Date().toISOString 
    }
  }, 
  db
)`

export const go_1 = `package main

import (
\t"bytes"
\t"fmt"
\t"io/ioutil"
\t"net/http"
\t"os"
)

func main() {
\tapiURL := "https://api.rushdb.com/api/v1/..."
\ttoken := os.Getenv("API_TOKEN")
\tdata := []byte(\`...\`)

\treq, err := http.NewRequest("POST", apiURL, bytes.NewBuffer(data))
\tif err != nil {
\t\tfmt.Println("Error creating request:", err)
\t\treturn
\t}

\treq.Header.Set("Content-Type", "application/json")
\treq.Header.Set("Token", token)

\tclient := &http.Client{}
\tresp, err := client.Do(req)
\tif err != nil {
\t\tfmt.Println("Error sending request:", err)
\t\treturn
\t}
\tdefer resp.Body.Close()

\tbody, _ := ioutil.ReadAll(resp.Body)
\tfmt.Println("Response:", string(body))
}`

export const rust_1 = `use reqwest::blocking::Client;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_url = "https://api.rushdb.com/api/v1/...";
    let token = env::var("API_TOKEN").expect("API_TOKEN must be set");
    let data = "...";

    let client = Client::new();
    let response = client.post(api_url)
        .header("Content-Type", "application/json")
        .header("Token", token)
        .body(data)
        .send()?;

    let body = response.text()?;
    println!("Response: {}", body);

    Ok(())
}`

export const php_1 = `<?php
$api_url = "https://api.rushdb.com/api/v1/...";
$token = getenv('API_TOKEN');
$data = '...';

$ch = curl_init($api_url);
curl_setopt($ch, CURLOPT_HTTPHEADER, [
    'Content-Type: application/json',
    "Token: $token"
]);
curl_setopt($ch, CURLOPT_POSTFIELDS, $data);
curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);

$response = curl_exec($ch);
curl_close($ch);

echo "Response: $response\\n";
?>`

export const java_1 = `import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Paths;

public class ApiRequest {
    public static void main(String[] args) throws Exception {
        String apiUrl = "https://api.rushdb.com/api/v1/...";
        String token = System.getenv("API_TOKEN");
        String data = "...";

        HttpClient client = HttpClient.newHttpClient();
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(apiUrl))
                .header("Content-Type", "application/json")
                .header("Token", token)
                .POST(HttpRequest.BodyPublishers.ofString(data))
                .build();

        HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());
        System.out.println("Response: " + response.body());
    }
}`

export const python_1 = `import os
import requests

api_url = "https://api.rushdb.com/api/v1/..."
token = os.getenv("API_TOKEN")
data = "..."

headers = {
    "Content-Type": "application/json",
    "Token": token
}

response = requests.post(api_url, headers=headers, data=data)

print("Response:", response.text)`

export const swift_1 = `import Foundation

let apiUrl = URL(string: "https://api.rushdb.com/api/v1/...")!
let token = ProcessInfo.processInfo.environment["API_TOKEN"] ?? ""
let data = Data("...".utf8)

var request = URLRequest(url: apiUrl)
request.httpMethod = "POST"
request.setValue("application/json", forHTTPHeaderField: "Content-Type")
request.setValue(token, forHTTPHeaderField: "Token")
request.httpBody = data

let task = URLSession.shared.dataTask(with: request) { data, response, error in
    guard let data = data, error == nil else {
        print("Error:", error ?? "Unknown error")
        return
    }

    let responseString = String(data: data, encoding: .utf8)
    print("Response:", responseString ?? "No response")
}
task.resume()
`

export const ruby_1 = `require 'net/http'
require 'json'
require 'uri'

api_url = URI.parse("https://api.rushdb.com/api/v1/...")
token = ENV['API_TOKEN']
data = '...'

http = Net::HTTP.new(api_url.host, api_url.port)
http.use_ssl = true

request = Net::HTTP::Post.new(api_url.request_uri, {
  'Content-Type' => 'application/json',
  'Token' => token
})
request.body = data

response = http.request(request)
puts "Response: #{response.body}"
`

//
export const curl_2 = `curl 
  -X POST 'https://api.rushdb.com/api/v1/records' \\
  -H 'Content-Type: application/json' \\
  -H "Token: $API_TOKEN" \\
  -d '{
    "label": "USER",
    "payload": {
      "email": "paul.schmitz@mail.com",
      "name": "Paul Schmitz",
      "age":  47
    }
  }'`

export const ts_2 = `// Create single Record
const user = await UserRepo.create({
  email: "paul.schmitz@mail.com",
  name: "Paul Schmitz",
  age: 47
})`

export const ts_2_1 = `// Create multiple Records at once
const catalog = await db.records.createMany(
  "CATEGORY", 
  [
    {
      title: "Sports and Travel",
      sidebarOrder: 5
      
      // Related Records
      PRODUCT: [
        {
          name: "Portable Gas Stove"
          price: 65
        },
        {
          name: "Sleeping Bag XL"
          price: 29
        },
      ]
    }
  ]
)`

export const go_2 = `package main

import (
\t"bytes"
\t"encoding/json"
\t"fmt"
\t"io/ioutil"
\t"net/http"
\t"os"
)

func main() {
\tapiURL := "https://api.rushdb.com/api/v1/records"
\ttoken := os.Getenv("API_TOKEN")

\tpayload := map[string]interface{}{
\t\t"label": "USER",
\t\t"payload": map[string]interface{}{
\t\t\t"email": "paul.schmitz@mail.com",
\t\t\t"name":  "Paul Schmitz",
\t\t\t"age":   47,
\t\t},
\t}

\tpayloadBytes, _ := json.Marshal(payload)
\treq, err := http.NewRequest("POST", apiURL, bytes.NewBuffer(payloadBytes))
\tif err != nil {
\t\tfmt.Println("Error creating request:", err)
\t\treturn
\t}

\treq.Header.Set("Content-Type", "application/json")
\treq.Header.Set("Token", token)

\tclient := &http.Client{}
\tresp, err := client.Do(req)
\tif err != nil {
\t\tfmt.Println("Error sending request:", err)
\t\treturn
\t}
\tdefer resp.Body.Close()

\tbody, _ := ioutil.ReadAll(resp.Body)
\tfmt.Println("Response:", string(body))
}
`

export const rust_2 = `use reqwest::blocking::Client;
use serde_json::json;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_url = "https://api.rushdb.com/api/v1/records";
    let token = env::var("API_TOKEN").expect("API_TOKEN must be set");

    let payload = json!({
        "label": "USER",
        "payload": {
            "email": "paul.schmitz@mail.com",
            "name": "Paul Schmitz",
            "age": 47
        }
    });

    let client = Client::new();
    let response = client.post(api_url)
        .header("Content-Type", "application/json")
        .header("Token", token)
        .json(&payload)
        .send()?;

    let body = response.text()?;
    println!("Response: {}", body);

    Ok(())
}`

export const php_2 = `<?php
$api_url = "https://api.rushdb.com/api/v1/records";
$token = getenv('API_TOKEN');

$data = json_encode([
    "label" => "USER",
    "payload" => [
        "email" => "paul.schmitz@mail.com",
        "name" => "Paul Schmitz",
        "age" => 47
    ]
]);

$ch = curl_init($api_url);
curl_setopt($ch, CURLOPT_HTTPHEADER, [
    'Content-Type: application/json',
    "Token: $token"
]);
curl_setopt($ch, CURLOPT_POSTFIELDS, $data);
curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);

$response = curl_exec($ch);
curl_close($ch);

echo "Response: $response\\n";
?>`

export const java_2 = `
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Paths;

public class ApiRequest {
    public static void main(String[] args) throws Exception {
        String apiUrl = "https://api.rushdb.com/api/v1/records";
        String token = System.getenv("API_TOKEN");

        String json = """
        {
            "label": "USER",
            "payload": {
                "email": "paul.schmitz@mail.com",
                "name": "Paul Schmitz",
                "age": 47
            }
        }
        """;

        HttpClient client = HttpClient.newHttpClient();
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(apiUrl))
                .header("Content-Type", "application/json")
                .header("Token", token)
                .POST(HttpRequest.BodyPublishers.ofString(json))
                .build();

        HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());
        System.out.println("Response: " + response.body());
    }
}`

export const python_2 = `import os
import requests
import json

api_url = "https://api.rushdb.com/api/v1/records"
token = os.getenv("API_TOKEN")

data = {
    "label": "USER",
    "payload": {
        "email": "paul.schmitz@mail.com",
        "name": "Paul Schmitz",
        "age": 47
    }
}

headers = {
    "Content-Type": "application/json",
    "Token": token
}

response = requests.post(api_url, headers=headers, data=json.dumps(data))

print("Response:", response.text)
`

export const swift_2 = `import Foundation

let apiUrl = URL(string: "https://api.rushdb.com/api/v1/records")!
let token = ProcessInfo.processInfo.environment["API_TOKEN"] ?? ""

let payload: [String: Any] = [
    "label": "USER",
    "payload": [
        "email": "paul.schmitz@mail.com",
        "name": "Paul Schmitz",
        "age": 47
    ]
]

let jsonData = try! JSONSerialization.data(withJSONObject: payload, options: [])

var request = URLRequest(url: apiUrl)
request.httpMethod = "POST"
request.setValue("application/json", forHTTPHeaderField: "Content-Type")
request.setValue(token, forHTTPHeaderField: "Token")
request.httpBody = jsonData

let task = URLSession.shared.dataTask(with: request) { data, response, error in
    guard let data = data, error == nil else {
        print("Error:", error ?? "Unknown error")
        return
    }

    let responseString = String(data: data, encoding: .utf8)
    print("Response:", responseString ?? "No response")
}
task.resume()`

export const ruby_2 = `require 'net/http'
require 'json'
require 'uri'

api_url = URI.parse("https://api.rushdb.com/api/v1/records")
token = ENV['API_TOKEN']

payload = {
  label: "USER",
  payload: {
    email: "paul.schmitz@mail.com",
    name: "Paul Schmitz",
    age: 47
  }
}.to_json

http = Net::HTTP.new(api_url.host, api_url.port)
http.use_ssl = true

request = Net::HTTP::Post.new(api_url.request_uri, {
  'Content-Type' => 'application/json',
  'Token' => token
})
request.body = payload

response = http.request(request)
puts "Response: #{response.body}"
`
//

export const curl_3 = `curl 
  -X POST 'https://api.rushdb.com/api/v1/records/search' \\
  -H 'Content-Type: application/json' \\
  -H "Token: $API_TOKEN" \\
  -d '{
    "labels": ["USER"],
    "where": {
      "name": { "$startsWith": "Paul" }
    },
    "orderBy": { "balance": "asc" }
  }'`

export const ts_3 = `// Basic search 
const users = await UserRepo.find({
  where: {
    name: { $startsWith: "Paul" }
  },
  orderBy: { balance: "asc" }
})`

export const ts_3_1 = `// Related search 
const orders = await OrderRepo.find({
  where: {
    sum: { $gt: 641 },
    PRODUCT: {
      brand: "Apple",
      CATEGORY: {
        title: "Accessories"
      }
    }
  }
})`

export const go_3 = `package main

import (
\t"bytes"
\t"encoding/json"
\t"fmt"
\t"io/ioutil"
\t"net/http"
\t"os"
)

func main() {
\tapiURL := "https://api.rushdb.com/api/v1/records/search"
\ttoken := os.Getenv("API_TOKEN")

\tpayload := map[string]interface{}{
\t\t"labels": []string{"USER"},
\t\t"where": map[string]interface{}{
\t\t\t"name": map[string]string{
\t\t\t\t"$startsWith": "Paul",
\t\t\t},
\t\t},
\t\t"orderBy": map[string]string{
\t\t\t"balance": "asc",
\t\t},
\t}

\tpayloadBytes, _ := json.Marshal(payload)
\treq, err := http.NewRequest("POST", apiURL, bytes.NewBuffer(payloadBytes))
\tif err != nil {
\t\tfmt.Println("Error creating request:", err)
\t\treturn
\t}

\treq.Header.Set("Content-Type", "application/json")
\treq.Header.Set("Token", token)

\tclient := &http.Client{}
\tresp, err := client.Do(req)
\tif err != nil {
\t\tfmt.Println("Error sending request:", err)
\t\treturn
\t}
\tdefer resp.Body.Close()

\tbody, _ := ioutil.ReadAll(resp.Body)
\tfmt.Println("Response:", string(body))
}`

export const rust_3 = `use reqwest::blocking::Client;
use serde_json::json;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_url = "https://api.rushdb.com/api/v1/records/search";
    let token = env::var("API_TOKEN").expect("API_TOKEN must be set");

    let payload = json!({
        "labels": ["USER"],
        "where": {
            "name": { "$startsWith": "Paul" }
        },
        "orderBy": { "balance": "asc" }
    });

    let client = Client::new();
    let response = client.post(api_url)
        .header("Content-Type", "application/json")
        .header("Token", token)
        .json(&payload)
        .send()?;

    let body = response.text()?;
    println!("Response: {}", body);

    Ok(())
}`

export const ruby_3 = `require 'net/http'
require 'json'
require 'uri'

api_url = URI.parse("https://api.rushdb.com/api/v1/records/search")
token = ENV['API_TOKEN']

payload = {
  labels: ["USER"],
  where: {
    name: { "$startsWith": "Paul" }
  },
  orderBy: { "balance": "asc" }
}.to_json

http = Net::HTTP.new(api_url.host, api_url.port)
http.use_ssl = true

request = Net::HTTP::Post.new(api_url.request_uri, {
  'Content-Type' => 'application/json',
  'Token' => token
})
request.body = payload

response = http.request(request)
puts "Response: #{response.body}"`

export const python_3 = `import os
import requests
import json

api_url = "https://api.rushdb.com/api/v1/records/search"
token = os.getenv("API_TOKEN")

data = {
    "labels": ["USER"],
    "where": {
        "name": { "$startsWith": "Paul" }
    },
    "orderBy": { "balance": "asc" }
}

headers = {
    "Content-Type": "application/json",
    "Token": token
}

response = requests.post(api_url, headers=headers, data=json.dumps(data))

print("Response:", response.text)`

export const swift_3 = `import Foundation

let apiUrl = URL(string: "https://api.rushdb.com/api/v1/records/search")!
let token = ProcessInfo.processInfo.environment["API_TOKEN"] ?? ""

let payload: [String: Any] = [
    "labels": ["USER"],
    "where": [
        "name": ["$startsWith": "Paul"]
    ],
    "orderBy": [
        "balance": "asc"
    ]
]

let jsonData = try! JSONSerialization.data(withJSONObject: payload, options: [])

var request = URLRequest(url: apiUrl)
request.httpMethod = "POST"
request.setValue("application/json", forHTTPHeaderField: "Content-Type")
request.setValue(token, forHTTPHeaderField: "Token")
request.httpBody = jsonData

let task = URLSession.shared.dataTask(with: request) { data, response, error in
    guard let data = data, error == nil else {
        print("Error:", error ?? "Unknown error")
        return
    }

    let responseString = String(data: data, encoding: .utf8)
    print("Response:", responseString ?? "No response")
}
task.resume()`

export const java_3 = `import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;

public class ApiRequest {
    public static void main(String[] args) throws Exception {
        String apiUrl = "https://api.rushdb.com/api/v1/records/search";
        String token = System.getenv("API_TOKEN");

        String json = """
        {
            "labels": ["USER"],
            "where": {
                "name": { "$startsWith": "Paul" }
            },
            "orderBy": { "balance": "asc" }
        }
        """;

        HttpClient client = HttpClient.newHttpClient();
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(apiUrl))
                .header("Content-Type", "application/json")
                .header("Token", token)
                .POST(HttpRequest.BodyPublishers.ofString(json))
                .build();

        HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());
        System.out.println("Response: " + response.body());
    }
}`

export const php_3 = `<?php
$api_url = "https://api.rushdb.com/api/v1/records/search";
$token = getenv('API_TOKEN');

$data = json_encode([
    "labels" => ["USER"],
    "where" => [
        "name" => [ "$startsWith" => "Paul" ]
    ],
    "orderBy" => [
        "balance" => "asc"
    ]
]);

$ch = curl_init($api_url);
curl_setopt($ch, CURLOPT_HTTPHEADER, [
    'Content-Type: application/json',
    "Token: $token"
]);
curl_setopt($ch, CURLOPT_POSTFIELDS, $data);
curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);

$response = curl_exec($ch);
curl_close($ch);

echo "Response: $response\\n";
?>`

//

export const transactionalAndSafeApiCodeBlock = `#!/bin/bash

# Obtain a Transaction
response=$(
  curl -X POST 'https://api.rushdb.com/api/v1/tx' \\
  -H 'Content-Type: application/json' \\
  -H "Token: $API_TOKEN" \\
  -d '{}'
)

# Extract an id from Transaction response
TX_ID=$(echo "$response" | jq -r '.id')

# Attach Transaction id to further requests
curl 'https://api.rushdb.com/api/v1/...' \\
  -H 'Content-Type: application/json' \\
  -H "Token: $API_TOKEN" \\
  -H "X-Transaction-Id: $TX_ID" \\
  -d '...'
  
# Commit Transaction
curl
  -X POST "https://api.rushdb.com/api/v1/tx/$TX_ID/commit" \\
  -H 'Content-Type: application/json' \\
  -H "Token: $API_TOKEN" \\
  -d '{}'
`

export const transactionalAndSafeCodeBlock = `// Start Transaction
const tx = await db.tx.begin() 

try {
  const order = await OrderRepo.create(
    {...},
    tx  // <-- Transaction
  )
  
  const merchant = await MerchantRepo.findOne(
    {...},
    tx // <-- Transaction
  )
  
  const { balance } = merchant.data
  await merchant.update(
    {
      balance: balance + order.data.total
    }, 
    tx // <-- Transaction
  )
  
  // Commit Transaction ✅
  await tx.commit() 
  
} catch (error) {
  
  // Rollback Transaction if error occurred ❌
  await tx.rollback() 
}
`

//

export const deleteComplexApiCodeBlock = `curl 
  -X DELETE 'https://api.rushdb.com/api/v1/records' \\
  -H 'Content-Type: application/json' \\
  -H "Token: $API_TOKEN" \\
  -d '{
    "labels": ["COMMENT"],
    "where": {
      "text": {
        "$in": [ "^*%&#", "@#*%&#", "$#@&&%" ]
      },
      "USER": {
        "email": "rude.troll@mail.com"
      }
    }
  }'`

export const deleteComplexCodeBlock = `// Delete Records based on complex criteria 
await CommentsRepo.delete({
  where: {
    text: {
      $in: [ "^*%&#", "@#*%&#", "$#@&&%" ]
    },
    USER: {
      email: "rude.troll@mail.com"
    }
  }
})`
