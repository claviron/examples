package main

import (
	"fmt"
	"io"
	"math/rand/v2"
	"net/http"
	"strconv"
	"sync"
	"sync/atomic"
	"time"
)

func routes() http.Handler {
	mux := http.NewServeMux()
	var active, total atomic.Int64
	var bodies sync.Map
	mux.HandleFunc("GET /{$}", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/plain; charset=utf-8")
		io.WriteString(w, "Go application on Claviron\n")
	})
	mux.HandleFunc("GET /bench/plain", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/plain; charset=utf-8")
		io.WriteString(w, "ok\n")
	})
	mux.HandleFunc("GET /bench/json", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		io.WriteString(w, `{"message":"ok"}`)
	})
	mux.HandleFunc("POST /bench/echo", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/octet-stream")
		if _, err := io.Copy(w, r.Body); err != nil {
			panic(http.ErrAbortHandler)
		}
	})
	// Keep the Axum example's workload URL; the response identifies Go.
	workload := func(w http.ResponseWriter, r *http.Request) {
		query := r.URL.Query()
		var low, high uint64
		for _, bound := range []struct {
			key   string
			value *uint64
		}{{"min", &low}, {"max", &high}} {
			if raw, ok := query[bound.key]; ok {
				value, err := strconv.ParseUint(raw[0], 10, 64)
				if err != nil || value > uint64((1<<63-1)/int64(time.Millisecond)) {
					http.Error(w, "invalid wait bound", http.StatusBadRequest)
					return
				}
				*bound.value = value
			}
		}
		if !query.Has("max") {
			high = low
		}
		if low > high {
			low, high = high, low
		}
		payload := -1
		if query.Has("payload") {
			value, err := strconv.ParseUint(query.Get("payload"), 10, 64)
			if err != nil {
				http.Error(w, "invalid payload", http.StatusBadRequest)
				return
			}
			payload = int(min(value, 1024*1024))
		}
		received, err := io.Copy(io.Discard, r.Body)
		if err != nil {
			panic(http.ErrAbortHandler)
		}
		if high > 0 {
			wait := low
			if high > low {
				wait += rand.Uint64N(high - low + 1)
			}
			timer := time.NewTimer(time.Duration(wait) * time.Millisecond)
			defer timer.Stop()
			select {
			case <-timer.C:
			case <-r.Context().Done():
				return
			}
		}
		w.Header().Set("Content-Type", "application/json")
		w.Header().Set("Cache-Control", "no-store")
		w.Header().Set("X-Received", strconv.FormatInt(received, 10))
		body := `{"from":"go"}`
		if payload >= 0 {
			cached, ok := bodies.Load(payload)
			if !ok {
				const alphabet = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
				message := make([]byte, payload)
				for i := range message {
					message[i] = alphabet[rand.IntN(len(alphabet))]
				}
				cached, _ = bodies.LoadOrStore(payload, `{"from":"go","message":"`+string(message)+`"}`)
			}
			body = cached.(string)
		}
		io.WriteString(w, body)
	}
	mux.HandleFunc("GET /_bench/rust", workload)
	mux.HandleFunc("POST /_bench/rust", workload)
	mux.HandleFunc("GET /_bench/stats", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.Header().Set("Cache-Control", "no-store")
		fmt.Fprintf(w, `{"in_flight":%d,"total":%d,"sessions":0}`, active.Load(), total.Load())
	})
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		total.Add(1)
		active.Add(1)
		defer active.Add(-1)
		mux.ServeHTTP(w, r)
	})
}
