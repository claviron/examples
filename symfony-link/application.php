<?php
declare(strict_types=1);

require __DIR__.'/vendor/autoload.php';

$environment = getenv('APP_ENV') ?: 'prod';
$debug = filter_var(getenv('APP_DEBUG') ?: '0', FILTER_VALIDATE_BOOL);
Claviron\Symfony\ApplicationRunner::run(new App\Kernel($environment, $debug));
