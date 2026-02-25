<?php

$port = getenv('PORT') ?: 8080;

header('Content-Type: application/json');

$uri = parse_url($_SERVER['REQUEST_URI'], PHP_URL_PATH);

switch ($uri) {
    case '/':
        echo json_encode([
            'message' => 'Hello from vanilla PHP!',
            'version' => '1.0.0',
        ]);
        break;

    case '/health':
        echo json_encode([
            'status' => 'healthy',
            'uptime' => time(),
        ]);
        break;

    default:
        http_response_code(404);
        echo json_encode(['error' => 'Not found']);
        break;
}
