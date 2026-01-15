<?php

require __DIR__ . '/vendor/autoload.php';

use Psr\Http\Message\ResponseInterface as Response;
use Psr\Http\Message\ServerRequestInterface as Request;
use Slim\Factory\AppFactory;

$app = AppFactory::create();

$port = getenv('PORT') ?: 8000;
$dbUrl = getenv('DATABASE_URL') ?: 'mysql://localhost/myapp';

$app->get('/', function (Request $request, Response $response) {
    $data = [
        'message' => 'PHP API Server',
        'version' => '1.0.0',
        'endpoints' => ['/', '/health', '/users']
    ];
    $response->getBody()->write(json_encode($data));
    return $response->withHeader('Content-Type', 'application/json');
});

$app->get('/health', function (Request $request, Response $response) {
    $data = [
        'status' => 'healthy',
        'uptime' => time()
    ];
    $response->getBody()->write(json_encode($data));
    return $response->withHeader('Content-Type', 'application/json');
});

$app->get('/users', function (Request $request, Response $response) {
    $users = [
        ['id' => 1, 'name' => 'Alice', 'email' => 'alice@example.com'],
        ['id' => 2, 'name' => 'Bob', 'email' => 'bob@example.com']
    ];
    $response->getBody()->write(json_encode(['users' => $users]));
    return $response->withHeader('Content-Type', 'application/json');
});

$app->post('/users', function (Request $request, Response $response) {
    $data = json_decode($request->getBody()->getContents(), true);
    $newUser = [
        'id' => 3,
        'name' => $data['name'] ?? '',
        'email' => $data['email'] ?? ''
    ];
    $response->getBody()->write(json_encode(['user' => $newUser]));
    return $response
        ->withHeader('Content-Type', 'application/json')
        ->withStatus(201);
});

$app->run();
