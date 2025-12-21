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
        $appEnv = %env(APP_ENV)%;
        $appPort = %env(APP_PORT)%;

        return $this->json([
            'status' => 'healthy',
            'environment' => $appEnv,
            'port' => $appPort
        ]);
    }
}
