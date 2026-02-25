<?php

$uri = urldecode(
    parse_url($_SERVER['REQUEST_URI'], PHP_URL_PATH)
);

// Handle health check directly without full framework bootstrap
if ($uri === '/health') {
    header('Content-Type: application/json');
    echo json_encode(['status' => 'healthy']);
    exit;
}

if ($uri !== '/' && file_exists(__DIR__.'/public'.$uri)) {
    return false;
}

require_once __DIR__.'/public/index.php';
