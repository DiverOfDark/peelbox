(defproject my-app "0.1.0"
  :description "A simple web app"
  :dependencies [[org.clojure/clojure "1.11.1"]
                 [ring/ring-core "1.10.0"]
                 [ring/ring-jetty-adapter "1.10.0"]]
  :main my-app.core
  :profiles {:uberjar {:aot :all}})
