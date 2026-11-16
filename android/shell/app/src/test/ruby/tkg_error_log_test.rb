# Run the real bootstrap and logger in an isolated temporary directory.
require 'tmpdir'
bootstrap = File.expand_path('../../main/assets/runtimepack_bootstrap.rb', __dir__)
Dir.mktmpdir('runtimepack-error-log-') do |dir|
  Dir.chdir(dir) do
    load bootstrap
    raise 'external encoding' unless Encoding.default_external == Encoding::UTF_8
    raise 'internal encoding' unless Encoding.default_internal == Encoding::UTF_8
    exception = RuntimeError.new('TKG logger behavioral sentinel')
    exception.set_backtrace(['example_script.rb:42:in `run`'])
    TKG::ErrorLog.save(exception)
    report = File.binread('Error.log')
    raise 'missing exception' unless report.include?('TKG logger behavioral sentinel')
    raise 'missing attribution' unless report.include?('example_script.rb:42')
    before = report.dup
    TKG::ErrorLog.save(nil)
    raise 'nil overwrote report' unless File.binread('Error.log') == before
  end
end
puts 'TKG error logger behavior PASS'
