require 'sinatra'
require 'json'

set :port, ENV.fetch('PORT', 4567).to_i
set :bind, '0.0.0.0'

configure do
  set :database_url, ENV.fetch('DATABASE_URL', 'postgres://localhost/myapp')
end

get '/' do
  content_type :json
  {
    message: 'Ruby API Server',
    version: '1.0.0',
    endpoints: ['/', '/health', '/users']
  }.to_json
end

get '/health' do
  content_type :json
  { status: 'healthy', uptime: Time.now.to_i }.to_json
end

get '/users' do
  content_type :json
  users = [
    { id: 1, name: 'Alice', email: 'alice@example.com' },
    { id: 2, name: 'Bob', email: 'bob@example.com' }
  ]
  { users: users }.to_json
end

post '/users' do
  content_type :json
  request.body.rewind
  data = JSON.parse(request.body.read)

  new_user = {
    id: 3,
    name: data['name'],
    email: data['email']
  }

  status 201
  { user: new_user }.to_json
end
