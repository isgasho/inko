# frozen_string_literal: true

require 'spec_helper'

describe Inkoc::Lexer do
  describe '#advance' do
    let(:lexer) { described_class.new('hello') }

    it 'returns a Token when there is remaining input' do
      token = lexer.advance

      expect(token).to be_an_instance_of(Inkoc::Token)
    end

    it 'returns a NullToken when there is no remaining input' do
      lexer.advance

      token = lexer.advance

      expect(token).to be_nil
      expect(token).to be_an_instance_of(Inkoc::NullToken)
    end

    it 'consumes a peeked value' do
      lexer.peek

      expect(lexer.advance).to be_an_instance_of(Inkoc::Token)
      expect(lexer.advance).to be_an_instance_of(Inkoc::NullToken)
    end

    it 'tokenizes a range' do
      lexer = described_class.new('1..2')

      [
        [:integer, '1'],
        [:inclusive_range, '..'],
        [:integer, '2']
      ].each do |(type, value)|
        token = lexer.advance

        expect(token.type).to eq(type)
        expect(token.value).to eq(value)
      end
    end

    it 'tokenizes a quoted backslash inside a block' do
      lexer = described_class.new("{ '\\\\' }")

      expect(lexer.advance.type).to eq(:curly_open)

      string = lexer.advance

      expect(string.type).to eq(:string)
      expect(string.value).to eq('\\')

      expect(lexer.advance.type).to eq(:curly_close)
    end
  end

  describe '#peek' do
    let(:lexer) { described_class.new('hello') }

    it 'peeks a token' do
      expect(lexer.peek).to be_an_instance_of(Inkoc::Token)
    end

    it 'does not consume already peeked values' do
      2.times do
        expect(lexer.peek).to be_an_instance_of(Inkoc::Token)
        expect(lexer.peek).to be_an_instance_of(Inkoc::Token)
      end
    end
  end

  describe '#skip_and_advance' do
    it 'skips a token and advances to the next one' do
      lexer = described_class.new('hello World')
      token = lexer.skip_and_advance

      expect(token).to be_an_instance_of(Inkoc::Token)
      expect(token.type).to eq(:constant)
      expect(token.value).to eq('World')
    end
  end

  describe '#next_type_is?' do
    let(:lexer) { described_class.new('hello') }

    it 'returns true when the next token is of a given type' do
      expect(lexer.next_type_is?(:identifier)).to eq(true)
    end

    it 'returns false when the next token is not of a given type' do
      expect(lexer.next_type_is?(:foo)).to eq(false)
    end

    it 'returns false when there is no token remaining' do
      lexer.advance

      expect(lexer.next_type_is?(:identifier)).to eq(false)
    end
  end

  describe '#advance_raw' do
    {
      'foo' => :identifier,
      'Foo' => :constant,
      '_foo' => :identifier,
      '_Foo' => :constant
    }.each do |input, type|
      it "tokenizes #{input.inspect}" do
        token = described_class.new(input).advance_raw

        expect(token.type).to eq(type)
      end
    end
  end

  describe '#carriage_return' do
    it 'advances to the next line' do
      lexer = described_class.new("\r")
      lexer.carriage_return

      expect(lexer.line).to eq(2)
      expect(lexer.column).to eq(1)
    end

    it 'skips over a newline if it follows the carriage return' do
      lexer = described_class.new("\r\n")
      lexer.carriage_return

      expect(lexer.line).to eq(2)
      expect(lexer.column).to eq(1)
    end
  end

  describe '#starts_with_underscore' do
    it 'tokenizes an identifier' do
      lexer = described_class.new('_foo')
      token = lexer.starts_with_underscore

      expect(token.type).to eq(:identifier)
      expect(token.value).to eq('_foo')
    end

    it 'tokenizes a constant' do
      lexer = described_class.new('_Foo')
      token = lexer.starts_with_underscore

      expect(token.type).to eq(:constant)
      expect(token.value).to eq('_Foo')
    end

    it 'returns a null token when out of input' do
      lexer = described_class.new('')
      token = lexer.starts_with_underscore

      expect(token).to be_nil
    end
  end

  describe '#identifier_or_keyword' do
    it 'tokenizes an identifier' do
      lexer = described_class.new('foo')
      token = lexer.identifier_or_keyword

      expect(token.type).to eq(:identifier)
      expect(token.value).to eq('foo')
    end

    it 'tokenizes a keyword' do
      lexer = described_class.new('try')
      token = lexer.identifier_or_keyword

      expect(token.type).to eq(:try)
    end

    it 'returns a null token when out of input' do
      lexer = described_class.new('')
      token = lexer.identifier_or_keyword

      expect(token).to be_nil
    end
  end

  describe '#constant' do
    it 'returns a token' do
      lexer = described_class.new('Foo')
      token = lexer.constant

      expect(token.type).to eq(:constant)
      expect(token.value).to eq('Foo')
    end

    it 'returns a null token when out of input' do
      lexer = described_class.new('')
      token = lexer.constant

      expect(token).to be_nil
    end
  end

  describe '#attribute' do
    it 'returns a token' do
      lexer = described_class.new('@foo')
      token = lexer.attribute

      expect(token.type).to eq(:attribute)
      expect(token.value).to eq('@foo')
    end
  end

  describe '#number' do
    it 'tokenizes an integer' do
      lexer = described_class.new('10')
      token = lexer.number

      expect(token.type).to eq(:integer)
      expect(token.value).to eq('10')
    end

    it 'tokenizes an integer with an underscore' do
      lexer = described_class.new('10_0')
      token = lexer.number

      expect(token.type).to eq(:integer)
      expect(token.value).to eq('100')
    end

    it 'tokenizes an float' do
      lexer = described_class.new('10.5')
      token = lexer.number

      expect(token.type).to eq(:float)
      expect(token.value).to eq('10.5')
    end

    it 'tokenizes an float with an underscore' do
      lexer = described_class.new('10_0.5')
      token = lexer.number

      expect(token.type).to eq(:float)
      expect(token.value).to eq('100.5')
    end

    it 'tokenizes a hexadecimal integer' do
      lexer = described_class.new('0x10')
      token = lexer.number

      expect(token.type).to eq(:integer)
      expect(token.value).to eq('0x10')
    end

    it 'tokenizes a hexadecimal integer with letters' do
      lexer = described_class.new('0xFF')
      token = lexer.number

      expect(token.type).to eq(:integer)
      expect(token.value).to eq('0xFF')
    end

    it 'tokenizes a float using the scientific notation with a lowercase e' do
      lexer = described_class.new('1e2')
      token = lexer.number

      expect(token.type).to eq(:float)
      expect(token.value).to eq('1e2')
    end

    it 'tokenizes a float using the scientific notation with an uppercase E' do
      lexer = described_class.new('1E2')
      token = lexer.number

      expect(token.type).to eq(:float)
      expect(token.value).to eq('1E2')
    end

    it 'tokenizes a float using the scientific notation with a plus sign' do
      lexer = described_class.new('1e+2')
      token = lexer.number

      expect(token.type).to eq(:float)
      expect(token.value).to eq('1e+2')
    end

    it 'tokenizes a hexadecimal integer containing the letter "e"' do
      lexer = described_class.new('0x1e2')
      token = lexer.number

      expect(token.type).to eq(:integer)
      expect(token.value).to eq('0x1e2')
    end
  end

  describe '#curly_open' do
    it 'tokenizes an opening curly brace' do
      lexer = described_class.new('{')
      token = lexer.curly_open

      expect(token.type).to eq(:curly_open)
      expect(token.value).to eq('{')
    end
  end

  describe '#curly_close' do
    it 'tokenizes an closeing curly brace' do
      lexer = described_class.new(')')
      token = lexer.curly_close

      expect(token.type).to eq(:curly_close)
      expect(token.value).to eq(')')
    end
  end

  describe '#paren_open' do
    it 'tokenizes an opening paren brace' do
      lexer = described_class.new('(')
      token = lexer.paren_open

      expect(token.type).to eq(:paren_open)
      expect(token.value).to eq('(')
    end
  end

  describe '#paren_close' do
    it 'tokenizes an closeing paren brace' do
      lexer = described_class.new(')')
      token = lexer.paren_close

      expect(token.type).to eq(:paren_close)
      expect(token.value).to eq(')')
    end
  end

  describe '#single_string' do
    it 'tokenizes a single quoted string' do
      lexer = described_class.new("'hello'")
      token = lexer.single_string

      expect(token.type).to eq(:string)
      expect(token.value).to eq('hello')
    end

    it 'tokenizes a single quoted string with an escaped quote' do
      lexer = described_class.new("'hello\\'world'")
      token = lexer.single_string

      expect(token.type).to eq(:string)
      expect(token.value).to eq("hello'world")
    end
  end

  describe '#double_string' do
    it 'tokenizes a double quoted string' do
      lexer = described_class.new('"hello"')
      token = lexer.double_string

      expect(token.type).to eq(:string)
      expect(token.value).to eq('hello')
    end

    it 'tokenizes a double quoted string with an escaped quote' do
      lexer = described_class.new('"hello\\"world"')
      token = lexer.double_string

      expect(token.type).to eq(:string)
      expect(token.value).to eq('hello"world')
    end

    it 'tokenizes a double quoted string with a newline' do
      lexer = described_class.new('"\n"')
      token = lexer.double_string

      expect(token.type).to eq(:string)
      expect(token.value).to eq("\n")
    end

    it 'tokenizes a double quoted string with a NULL byte' do
      lexer = described_class.new('"\0"')
      token = lexer.double_string

      expect(token.type).to eq(:string)
      expect(token.value).to eq("\0")
    end

    it 'tokenizes a double quoted string with a carriage return' do
      lexer = described_class.new('"\r"')
      token = lexer.double_string

      expect(token.type).to eq(:string)
      expect(token.value).to eq("\r")
    end

    it 'tokenizes a double quoted string with a tab' do
      lexer = described_class.new('"\t"')
      token = lexer.double_string

      expect(token.type).to eq(:string)
      expect(token.value).to eq("\t")
    end
  end

  describe 'template strings' do
    it 'tokenizes a template string' do
      lexer = described_class.new('`Hello \n {10} world`')

      tok1 = lexer.advance
      tok2 = lexer.advance
      tok3 = lexer.advance
      tok4 = lexer.advance
      tok5 = lexer.advance
      tok6 = lexer.advance
      tok7 = lexer.advance

      expect(tok1.type).to eq(:tstring_open)
      expect(tok2.type).to eq(:tstring_body)
      expect(tok2.value).to eq("Hello \n ")
      expect(tok3.type).to eq(:tstring_expr_open)
      expect(tok4.type).to eq(:integer)
      expect(tok5.type).to eq(:tstring_expr_close)
      expect(tok6.type).to eq(:tstring_body)
      expect(tok6.value).to eq(' world')
      expect(tok7.type).to eq(:tstring_close)
    end
  end

  describe '#colons' do
    it 'tokenizes a single colon' do
      lexer = described_class.new(':')
      token = lexer.colons

      expect(token.type).to eq(:colon)
      expect(token.value).to eq(':')
    end

    it 'tokenizes a double colon' do
      lexer = described_class.new('::')
      token = lexer.colons

      expect(token.type).to eq(:colon_colon)
      expect(token.value).to eq('::')
    end
  end

  describe '#div' do
    it 'tokenizes the division operator' do
      lexer = described_class.new('/')
      token = lexer.div

      expect(token.type).to eq(:div)
      expect(token.value).to eq('/')
    end

    it 'tokenizes the division-assign operator' do
      lexer = described_class.new('/=')
      token = lexer.div

      expect(token.type).to eq(:div_assign)
      expect(token.value).to eq('/=')
    end
  end

  describe '#modulo' do
    it 'tokenizes the modulo operator' do
      lexer = described_class.new('%')
      token = lexer.modulo

      expect(token.type).to eq(:mod)
      expect(token.value).to eq('%')
    end

    it 'tokenizes the module-assign operator' do
      lexer = described_class.new('%=')
      token = lexer.modulo

      expect(token.type).to eq(:mod_assign)
      expect(token.value).to eq('%=')
    end
  end

  describe '#bitwise_xor' do
    it 'tokenizes the bitwise XOR operator' do
      lexer = described_class.new('^')
      token = lexer.bitwise_xor

      expect(token.type).to eq(:bitwise_xor)
      expect(token.value).to eq('^')
    end

    it 'tokenizes the bitwise XOR assign operator' do
      lexer = described_class.new('^=')
      token = lexer.bitwise_xor

      expect(token.type).to eq(:bitwise_xor_assign)
      expect(token.value).to eq('^=')
    end
  end

  describe '#bitwise_and_or_boolean_and' do
    it 'tokenizes the bitwise AND operator' do
      lexer = described_class.new('&')
      token = lexer.bitwise_and_or_boolean_and

      expect(token.type).to eq(:bitwise_and)
      expect(token.value).to eq('&')
    end

    it 'tokenizes the bitwise AND-assign operator' do
      lexer = described_class.new('&=')
      token = lexer.bitwise_and_or_boolean_and

      expect(token.type).to eq(:bitwise_and_assign)
      expect(token.value).to eq('&=')
    end

    it 'tokenizes the AND operator' do
      lexer = described_class.new('&&')
      token = lexer.bitwise_and_or_boolean_and

      expect(token.type).to eq(:and)
      expect(token.value).to eq('&&')
    end
  end

  describe '#bitwise_or_or_boolean_or' do
    it 'tokenizes the bitwise OR operator' do
      lexer = described_class.new('|')
      token = lexer.bitwise_or_or_boolean_or

      expect(token.type).to eq(:bitwise_or)
      expect(token.value).to eq('|')
    end

    it 'tokenizes the bitwise OR-assign operator' do
      lexer = described_class.new('|=')
      token = lexer.bitwise_or_or_boolean_or

      expect(token.type).to eq(:bitwise_or_assign)
      expect(token.value).to eq('|=')
    end

    it 'tokenizes the OR operator' do
      lexer = described_class.new('||')
      token = lexer.bitwise_or_or_boolean_or

      expect(token.type).to eq(:or)
      expect(token.value).to eq('||')
    end
  end

  describe '#mul_or_pow' do
    it 'tokenizes the multiplication operator' do
      lexer = described_class.new('*')
      token = lexer.mul_or_pow

      expect(token.type).to eq(:mul)
      expect(token.value).to eq('*')
    end

    it 'tokenizes the multiplication-assign operator' do
      lexer = described_class.new('*=')
      token = lexer.mul_or_pow

      expect(token.type).to eq(:mul_assign)
      expect(token.value).to eq('*=')
    end

    it 'tokenizes the power operator' do
      lexer = described_class.new('**')
      token = lexer.mul_or_pow

      expect(token.type).to eq(:pow)
      expect(token.value).to eq('**')
    end

    it 'tokenizes the power-assign operator' do
      lexer = described_class.new('**=')
      token = lexer.mul_or_pow

      expect(token.type).to eq(:pow_assign)
      expect(token.value).to eq('**=')
    end
  end

  describe '#sub_or_arrow_or_negative_number' do
    it 'tokenizes the subtraction operator' do
      lexer = described_class.new('-')
      token = lexer.sub_or_arrow_or_negative_number

      expect(token.type).to eq(:sub)
      expect(token.value).to eq('-')
    end

    it 'tokenizes the subtraction-assign operator' do
      lexer = described_class.new('-=')
      token = lexer.sub_or_arrow_or_negative_number

      expect(token.type).to eq(:sub_assign)
      expect(token.value).to eq('-=')
    end

    it 'tokenizes the arrow operator' do
      lexer = described_class.new('->')
      token = lexer.sub_or_arrow_or_negative_number

      expect(token.type).to eq(:arrow)
      expect(token.value).to eq('->')
    end

    it 'tokenizes a negative integer' do
      lexer = described_class.new('-10')
      token = lexer.advance

      expect(token.type).to eq(:integer)
      expect(token.value).to eq('-10')
    end

    it 'requires no whitespace after a sign to tokenize a negative integer' do
      lexer = described_class.new('- 10')
      token1 = lexer.advance
      token2 = lexer.advance

      expect(token1.type).to eq(:sub)
      expect(token1.value).to eq('-')

      expect(token2.type).to eq(:integer)
      expect(token2.value).to eq('10')
    end
  end

  describe '#add' do
    it 'tokenizes the addition operator' do
      lexer = described_class.new('+')
      token = lexer.add

      expect(token.type).to eq(:add)
      expect(token.value).to eq('+')
    end

    it 'tokenizes the addition-assign operator' do
      lexer = described_class.new('+=')
      token = lexer.add

      expect(token.type).to eq(:add_assign)
      expect(token.value).to eq('+=')
    end
  end

  describe '#assign_or_equal' do
    it 'tokenizes the assignment operator' do
      lexer = described_class.new('=')
      token = lexer.assign_or_equal

      expect(token.type).to eq(:assign)
      expect(token.value).to eq('=')
    end

    it 'tokenizes the equality operator' do
      lexer = described_class.new('==')
      token = lexer.assign_or_equal

      expect(token.type).to eq(:equal)
      expect(token.value).to eq('==')
    end

    it 'tokenizes the double arrow sign' do
      lexer = described_class.new('=>')
      token = lexer.assign_or_equal

      expect(token.type).to eq(:darrow)
      expect(token.value).to eq('=>')
    end
  end

  describe '#not_equal_or_type_args_open_or_throws' do
    it 'tokenizes the not-equal operator' do
      lexer = described_class.new('!=')
      token = lexer.not_equal_or_type_args_open_or_throws

      expect(token.type).to eq(:not_equal)
      expect(token.value).to eq('!=')
    end

    it 'tokenizes the type arguments open token' do
      lexer = described_class.new('!(')
      token = lexer.not_equal_or_type_args_open_or_throws

      expect(token.type).to eq(:type_args_open)
      expect(token.value).to eq('!(')
    end

    it 'tokenizes the throws token' do
      lexer = described_class.new('!!')
      token = lexer.not_equal_or_type_args_open_or_throws

      expect(token.type).to eq(:throws)
      expect(token.value).to eq('!!')
    end

    it 'tokenizes the exclamation suffix operator' do
      lexer = described_class.new('!')
      token = lexer.not_equal_or_type_args_open_or_throws

      expect(token.type).to eq(:exclamation)
      expect(token.value).to eq('!')
    end
  end

  describe '#dot_or_range' do
    it 'tokenizes the dot operator' do
      lexer = described_class.new('.')
      token = lexer.dot_or_range

      expect(token.type).to eq(:dot)
      expect(token.value).to eq('.')
    end

    it 'tokenizes the inclusive-range operator' do
      lexer = described_class.new('..')
      token = lexer.dot_or_range

      expect(token.type).to eq(:inclusive_range)
      expect(token.value).to eq('..')
    end

    it 'tokenizes the exclusive-range operator' do
      lexer = described_class.new('...')
      token = lexer.dot_or_range

      expect(token.type).to eq(:exclusive_range)
      expect(token.value).to eq('...')
    end
  end

  describe '#comma' do
    it 'tokenizes a comma' do
      lexer = described_class.new(',')
      token = lexer.comma

      expect(token.type).to eq(:comma)
      expect(token.value).to eq(',')
    end
  end

  describe '#lower_or_shift_left' do
    it 'tokenizes the lower-than operator' do
      lexer = described_class.new('<')
      token = lexer.lower_or_shift_left

      expect(token.type).to eq(:lower)
      expect(token.value).to eq('<')
    end

    it 'tokenizes the lower-than-or-equal operator' do
      lexer = described_class.new('<=')
      token = lexer.lower_or_shift_left

      expect(token.type).to eq(:lower_equal)
      expect(token.value).to eq('<=')
    end

    it 'tokenizes the shift-left operator' do
      lexer = described_class.new('<<')
      token = lexer.lower_or_shift_left

      expect(token.type).to eq(:shift_left)
      expect(token.value).to eq('<<')
    end

    it 'tokenizes the shift-left-assign operator' do
      lexer = described_class.new('<<=')
      token = lexer.lower_or_shift_left

      expect(token.type).to eq(:shift_left_assign)
      expect(token.value).to eq('<<=')
    end
  end

  describe '#greater_or_shift_right' do
    it 'tokenizes the greater-than operator' do
      lexer = described_class.new('>')
      token = lexer.greater_or_shift_right

      expect(token.type).to eq(:greater)
      expect(token.value).to eq('>')
    end

    it 'tokenizes the greater-than-or-equal operator' do
      lexer = described_class.new('>=')
      token = lexer.greater_or_shift_right

      expect(token.type).to eq(:greater_equal)
      expect(token.value).to eq('>=')
    end

    it 'tokenizes the shift-right operator' do
      lexer = described_class.new('>>')
      token = lexer.greater_or_shift_right

      expect(token.type).to eq(:shift_right)
      expect(token.value).to eq('>>')
    end

    it 'tokenizes the shift-right-assign operator' do
      lexer = described_class.new('>>=')
      token = lexer.greater_or_shift_right

      expect(token.type).to eq(:shift_right_assign)
      expect(token.value).to eq('>>=')
    end
  end

  describe '#bracket_open' do
    it 'tokenizes an opening square bracket' do
      lexer = described_class.new('[')
      token = lexer.bracket_open

      expect(token.type).to eq(:bracket_open)
      expect(token.value).to eq('[')
    end
  end

  describe '#bracket_close' do
    it 'tokenizes an closeing square bracket' do
      lexer = described_class.new(']')
      token = lexer.bracket_close

      expect(token.type).to eq(:bracket_close)
      expect(token.value).to eq(']')
    end
  end

  describe '#question_mark' do
    it 'tokenizes a question mark' do
      lexer = described_class.new('?')
      token = lexer.question_mark

      expect(token.type).to eq(:question)
      expect(token.value).to eq('?')
    end

    it 'tokenizes the nil coalescing operator' do
      lexer = described_class.new('??')
      token = lexer.question_mark

      expect(token.type).to eq(:question_question)
      expect(token.value).to eq('??')
    end

    it 'tokenizes a question mark separately at the start of an identifier' do
      lexer = described_class.new('?Foo')

      token1 = lexer.advance
      token2 = lexer.advance

      expect(token1.type).to eq(:question)
      expect(token1.value).to eq('?')

      expect(token2.type).to eq(:constant)
      expect(token2.value).to eq('Foo')
    end
  end

  it 'returns a null token for unrecognized input' do
    lexer = described_class.new(';')

    expect(lexer.advance).to be_nil
  end

  describe '#comment' do
    it 'tokenizes comments' do
      lexer = described_class.new('# foo')
      token = lexer.comment

      expect(token.type).to eq(:comment)
      expect(token.value).to eq('foo')
    end
  end
end
