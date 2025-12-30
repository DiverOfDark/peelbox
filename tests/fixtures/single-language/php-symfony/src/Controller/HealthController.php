<?php

namespace App\Controller;

use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\HttpFoundation\JsonResponse;
use Symfony\Component\Routing\Annotation\Route;

class HealthController extends AbstractController
{
    #[Route('/_health', name: 'health', methods: ['GET'])]
    public function health(): JsonResponse
    {
        $appEnv = $_ENV['APP_ENV'] ?? 'production';
        $appPort = $_ENV['APP_PORT'] ?? '8000';

        return $this->json([
            'status' => 'healthy',
            'environment' => $appEnv,
            'port' => $appPort
        ]);
    }
}
