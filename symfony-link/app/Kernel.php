<?php
declare(strict_types=1);

namespace App;

use Claviron\Bench\Symfony\BenchController;
use Symfony\Bundle\FrameworkBundle\FrameworkBundle;
use Symfony\Bundle\FrameworkBundle\Kernel\MicroKernelTrait;
use Symfony\Component\DependencyInjection\Loader\Configurator\ContainerConfigurator;
use Symfony\Component\HttpKernel\Kernel as BaseKernel;
use Symfony\Component\Routing\Loader\Configurator\RoutingConfigurator;

final class Kernel extends BaseKernel
{
    use MicroKernelTrait;

    public function getProjectDir(): string
    {
        return dirname(__DIR__);
    }

    public function registerBundles(): iterable
    {
        yield new FrameworkBundle();
    }

    private function configureContainer(ContainerConfigurator $container): void
    {
        $container->extension('framework', ['http_method_override' => false]);
        $container->services()->set(Controller::class)->public();
        // The shared benchmark workload (claviron/bench): the same paths and
        // bodies in every language.
        $container->services()->set(BenchController::class)
            ->autowire()->autoconfigure()->tag('controller.service_arguments');
    }

    private function configureRoutes(RoutingConfigurator $routes): void
    {
        $routes->add('home', '/')->controller(Controller::class)->methods(['GET']);
        $routes->add('echo', '/echo')->controller(Controller::class)->methods(['POST']);
        $routes->import(dirname(__DIR__).'/vendor/claviron/bench/src/Symfony/BenchController.php', 'attribute');
    }
}
