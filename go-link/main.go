package main

import (
	"context"
	"flag"
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/claviron/go/orbit"
)

func run() error {
	fleet := flag.String("fleet", "go", "Orbit fleet")
	name := flag.String("name", "go-link", "application name")
	inFlight := flag.Uint64("max-in-flight", 128, "maximum running exchanges")
	queued := flag.Uint64("max-queued", 0, "maximum waiting exchanges")
	flag.Func("max-exchanges", "replaced by --max-in-flight and --max-queued", func(string) error {
		return fmt.Errorf("--max-exchanges was replaced by --max-in-flight and --max-queued")
	})
	buffer := flag.Uint64("io-buffer-bytes", 8192, "bytes per native I/O operation")
	flag.Parse()
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	app, err := orbit.Open(orbit.Options{
		Fleet: *fleet, Name: *name, Incarnation: 1,
		NativeThreads: 1, MaxInFlight: *inFlight, MaxQueued: *queued, IOBufferBytes: *buffer,
	}, routes())
	if err != nil {
		return err
	}
	defer app.Close()
	fmt.Printf("%s joined fleet %s, lane %d\n", *name, *fleet, app.Lane())
	select {
	case <-ctx.Done():
		return app.Shutdown(context.Background())
	case <-app.Done():
		return app.Err()
	}
}

func main() {
	if err := run(); err != nil {
		log.Print(err)
		os.Exit(1)
	}
}
