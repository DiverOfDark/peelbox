require 'sinatra'

set :port, ENV.fetch('PORT', 4567).to_i
set :bind, '0.0.0.0'

get '/health' do
  content_type :json
  { status: 'ok' }.to_json
end

get '/' do
  content_type :json
  { message: 'Hello from Sinatra' }.to_json
end
