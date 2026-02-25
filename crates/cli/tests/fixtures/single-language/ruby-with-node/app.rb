require 'sinatra'
require 'json'

set :port, ENV.fetch('PORT', 4567).to_i
set :bind, '0.0.0.0'

get '/' do
  content_type :json
  {
    message: 'Ruby with Node.js',
    version: '1.0.0'
  }.to_json
end

get '/health' do
  content_type :json
  { status: 'healthy' }.to_json
end
