# frozen_string_literal: true

module Inkoc
  module Type
    module TypeCompatibility
      def implements_trait?(trait)
        trait = trait.type if trait.optional?

        if trait.type_parameter?
          trait.required_traits.all? { |t| implements_trait?(t) }
        else
          source = self

          while source
            return true if source.implemented_traits.include?(trait)

            source = source.prototype
          end

          false
        end
      end

      def implements_all_traits?(traits)
        traits.all? { |trait| implements_trait?(trait) }
      end

      def basic_type_compatibility?(other)
        return true if self == other || other.dynamic?
        return false if other.void?
        return implements_trait?(other) if other.trait?
        return type_compatible?(other.type) if other.optional?

        nil
      end

      # Returns true if the current and the given type are compatible.
      def type_compatible?(other)
        basic_compat = basic_type_compatibility?(other)

        if basic_compat.nil?
          # Generic types that are initialized set their prototype to the base
          # type, so in this case we also need to compare with the prototype of
          # the object we're comparing with.
          if other.generic_type?
            prototype == other || prototype == other.prototype ||
              prototype&.type_compatible?(other)
          else
            prototype == other
          end
        else
          basic_compat
        end
      end

      def strict_type_compatible?(other)
        return false if other.dynamic?

        type_compatible?(other)
      end
    end
  end
end
