name := "my-scala-app"
version := "0.1.0"
scalaVersion := "3.3.1"

assembly / mainClass := Some("com.example.Main")

libraryDependencies ++= Seq(
  "com.typesafe.akka" %% "akka-http" % "10.5.3",
  "com.typesafe.akka" %% "akka-stream" % "2.8.5"
)
