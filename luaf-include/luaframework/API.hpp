#pragma once

#include <cstdint>
#include <string>
#include <vector>
#include <format>
#include <memory>
#include <stdexcept>

#define LUAF_API __declspec(dllexport)

namespace luaf
{

	typedef void (*OnLuaStateCreatedCb)(void*);
	typedef void (*OnLuaStateDestroyedCb)(void*);

	typedef struct CoreAPIFunctions {
		void (*add_core_function)(const char*, uint32_t, const void*);
		const void* (*get_core_function)(const char*, uint32_t);
		const void* (*get_singleton)(const char*, uint32_t);
		void* (*get_managed_address)(const char*, uint32_t);
		void (*set_managed_address)(const char* name, uint32_t name_len, const char* pattern, uint32_t pattern_len, int offset);
	} CoreAPIFunctions;

	typedef struct CoreAPILua {
		void (*on_lua_state_created)(OnLuaStateCreatedCb);
		void (*on_lua_state_destroyed)(OnLuaStateDestroyedCb);
		void (*with_lua_lock)(void (*)(void*), void*);
	} CoreAPILua;

	typedef struct CoreAPIInput {
		bool (*is_key_pressed)(uint32_t);
		bool (*is_key_down)(uint32_t);
		bool (*is_controller_pressed)(uint32_t);
		bool (*is_controller_down)(uint32_t);
	} CoreAPIInput;

	typedef struct CoreAPIParam {
		const CoreAPIFunctions* functions;
		void (*log)(uint32_t, const char*, uint32_t);
		const CoreAPILua* lua;
		const CoreAPIInput* input;
	} CoreAPIParam;

	class Api
	{
	private:
		static inline std::unique_ptr<Api> s_instance{};

		const CoreAPIParam* m_param;

	public:
		Api(const CoreAPIParam* param)
			: m_param{ param }
		{
		}

		static std::unique_ptr<Api>& initialize(const CoreAPIParam* param)
		{
			if (param == nullptr)
			{
				throw std::runtime_error("param is null");
			}
			if (s_instance != nullptr)
			{
				throw std::runtime_error("API already initialized");
			}

			s_instance = std::make_unique<Api>(param);
			return s_instance;
		}

		static auto& get()
		{
			if (s_instance == nullptr)
			{
				throw std::runtime_error("API not initialized");
			}

			return s_instance;
		};

		enum Level : uint32_t
		{
			Trace = 0,
			Debug = 1,
			Info = 2,
			Warn = 3,
			Error = 4,
		};

		void log_to_logger(Level level, std::string_view msg) {
			m_param->log(level, msg.data(), 0);
		}

		template<typename TFunc>
		void add_core_function(std::string_view name, TFunc* fun) {
			m_param->add_core_function(name.data(), 0, fun);
		}

		template<typename TFunc>
		TFunc* get_core_function(std::string_view method) const {
			return static_cast<TFunc*>(m_param->get_core_function(method.data(), 0));
		}

	};

	// Static Logger, easier to use.
	class Log {
	public:
		template <typename... Args>
		static void trace(const std::format_string<Args...>& fmt, Args &&...args)
		{
			log(Api::Level::Trace, std::vformat(fmt.get(), std::make_format_args(args...)));
		}
		template <typename... Args>
		static void debug(const std::format_string<Args...>& fmt, Args &&...args)
		{
			log(Api::Level::Debug, std::vformat(fmt.get(), std::make_format_args(args...)));
		}
		template <typename... Args>
		static void info(const std::format_string<Args...>& fmt, Args &&...args)
		{
			log(Api::Level::Info, std::vformat(fmt.get(), std::make_format_args(args...)));
		}
		template <typename... Args>
		static void warn(const std::format_string<Args...>& fmt, Args &&...args)
		{
			log(Api::Level::Warn, std::vformat(fmt.get(), std::make_format_args(args...)));
		}
		template <typename... Args>
		static void error(const std::format_string<Args...>& fmt, Args &&...args)
		{
			log(Api::Level::Error, std::vformat(fmt.get(), std::make_format_args(args...)));
		}

	private:
		static void log(Api::Level level, std::string_view msg) {
			Api::get()->log_to_logger(level, msg);
		}
	};
}