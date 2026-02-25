(ns my-app.core
  (:require [ring.adapter.jetty :refer [run-jetty]]
            [ring.util.response :refer [response]])
  (:gen-class))

(defn handler [request]
  (response "Hello, World!"))

(defn -main [& args]
  (run-jetty handler {:port 3000}))
